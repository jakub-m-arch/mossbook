//! Voice notes (E-18, decision D-21): a recording attached to an entry.
//!
//! The recording itself is an Ogg Opus file (RFC 3533, RFC 7845) kept in the
//! `media` table; an entry only points to it. The journal checks the file's
//! structure and takes the length from it, so the UI cannot claim a different
//! one or store something that is not a recording.

use core::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use super::{DomainError, Id};

/// The only format stored: Ogg with one Opus stream.
pub const VOICE_MIME: &str = "audio/ogg; codecs=opus";
/// Longest recording, in milliseconds (5 minutes, D-21).
pub const MAX_VOICE_MS: u32 = 5 * 60 * 1000;
/// Slack over [`MAX_VOICE_MS`] for the encoder's last frame and padding.
const LENGTH_SLACK_MS: u32 = 1000;
/// Largest recording in bytes. Five minutes of speech at the 32 kbit/s the app
/// records at is about 1.2 MB; the rest is headroom for Ogg overhead.
pub const MAX_VOICE_BYTES: usize = 4 * 1024 * 1024;

/// Opus always counts granule positions at 48 kHz (RFC 7845 §4).
const OPUS_RATE: u64 = 48_000;

/// A recording attached to an entry.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
pub struct VoiceNote {
    /// Row id in the `media` table.
    pub id: Id,
    /// Length taken from the file when it was stored.
    pub duration_ms: u32,
    #[serde(default = "default_mime")]
    pub mime: String,
}

fn default_mime() -> String {
    VOICE_MIME.to_owned()
}

impl fmt::Debug for VoiceNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceNote")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Checks that `bytes` is a complete Ogg Opus file within the limits and
/// returns its length in milliseconds.
pub fn inspect_ogg_opus(bytes: &[u8]) -> Result<u32, DomainError> {
    const INVALID: DomainError = DomainError::InvalidField("voice");
    if bytes.len() > MAX_VOICE_BYTES {
        return Err(INVALID);
    }
    let mut rest = bytes;
    let mut serial = None;
    let mut sequence = 0u32;
    let mut pre_skip = None;
    let mut last_granule = None;
    let mut ended = false;
    while !rest.is_empty() {
        if ended {
            return Err(INVALID); // nothing may follow the last page
        }
        let page = OggPage::parse(rest).ok_or(INVALID)?;
        rest = &rest[page.len..];
        if *serial.get_or_insert(page.serial) != page.serial || page.sequence != sequence {
            return Err(INVALID);
        }
        let first = sequence == 0;
        if first != page.begins_stream() || page.continues_packet() {
            return Err(INVALID);
        }
        match sequence {
            0 => pre_skip = Some(opus_head(page.body).ok_or(INVALID)?),
            1 if !page.body.starts_with(b"OpusTags") => return Err(INVALID),
            _ => {}
        }
        if sequence >= 2 && page.granule != u64::MAX {
            last_granule = Some(page.granule);
        }
        ended = page.ends_stream();
        sequence = sequence.checked_add(1).ok_or(INVALID)?;
    }
    let (Some(pre_skip), Some(granule), true) = (pre_skip, last_granule, ended) else {
        return Err(INVALID);
    };
    let samples = granule.checked_sub(u64::from(pre_skip)).ok_or(INVALID)?;
    let ms = u32::try_from(samples * 1000 / OPUS_RATE).map_err(|_| INVALID)?;
    if ms == 0 || ms > MAX_VOICE_MS + LENGTH_SLACK_MS {
        return Err(INVALID);
    }
    Ok(ms.min(MAX_VOICE_MS))
}

/// The identification header (RFC 7845 §5.1); returns the pre-skip.
fn opus_head(body: &[u8]) -> Option<u16> {
    if body.len() < 19 || !body.starts_with(b"OpusHead") {
        return None;
    }
    let version = body[8];
    let channels = body[9];
    // Major version 0 only; mono or stereo with the default channel mapping.
    if version >> 4 != 0 || !(1..=2).contains(&channels) || body[18] != 0 {
        return None;
    }
    Some(u16::from_le_bytes([body[10], body[11]]))
}

struct OggPage<'a> {
    flags: u8,
    granule: u64,
    serial: u32,
    sequence: u32,
    body: &'a [u8],
    len: usize,
}

impl<'a> OggPage<'a> {
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        let header = bytes.get(..27)?;
        if &header[..4] != b"OggS" || header[4] != 0 {
            return None;
        }
        let segments = usize::from(header[26]);
        let table = bytes.get(27..27 + segments)?;
        let body_len: usize = table.iter().map(|&s| usize::from(s)).sum();
        let len = 27 + segments + body_len;
        let page = bytes.get(..len)?;
        let stored = u32::from_le_bytes(header[22..26].try_into().ok()?);
        if ogg_crc(page) != stored {
            return None;
        }
        Some(Self {
            flags: header[5],
            granule: u64::from_le_bytes(header[6..14].try_into().ok()?),
            serial: u32::from_le_bytes(header[14..18].try_into().ok()?),
            sequence: u32::from_le_bytes(header[18..22].try_into().ok()?),
            body: &page[27 + segments..],
            len,
        })
    }

    fn continues_packet(&self) -> bool {
        self.flags & 0x01 != 0
    }

    fn begins_stream(&self) -> bool {
        self.flags & 0x02 != 0
    }

    fn ends_stream(&self) -> bool {
        self.flags & 0x04 != 0
    }
}

/// The Ogg page checksum (RFC 3533 §6: CRC-32, polynomial 0x04c11db7, no
/// reflection), computed with the checksum field taken as zero. An integrity
/// check of the container, not cryptography.
fn ogg_crc(page: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut r = (i as u32) << 24;
            let mut bit = 0;
            while bit < 8 {
                r = if r & 0x8000_0000 != 0 {
                    (r << 1) ^ 0x04c1_1db7
                } else {
                    r << 1
                };
                bit += 1;
            }
            table[i] = r;
            i += 1;
        }
        table
    };
    let mut crc = 0u32;
    for (i, &b) in page.iter().enumerate() {
        let b = if (22..26).contains(&i) { 0 } else { b };
        crc = (crc << 8) ^ TABLE[usize::from(((crc >> 24) as u8) ^ b)];
    }
    crc
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic Ogg Opus file: `packets` packets of 20 ms (960 samples) of
    /// arbitrary bytes, one per page. Not decodable audio, but a valid container.
    pub(crate) fn ogg_opus(packets: usize) -> Vec<u8> {
        let pre_skip = 312u16;
        let mut head = b"OpusHead".to_vec();
        head.extend_from_slice(&[1, 1]);
        head.extend_from_slice(&pre_skip.to_le_bytes());
        head.extend_from_slice(&48_000u32.to_le_bytes());
        head.extend_from_slice(&[0, 0, 0]);
        let mut tags = b"OpusTags".to_vec();
        tags.extend_from_slice(&4u32.to_le_bytes());
        tags.extend_from_slice(b"test");
        tags.extend_from_slice(&0u32.to_le_bytes());
        let mut out = page(0x02, 0, 0, &head);
        out.extend(page(0, 0, 1, &tags));
        for i in 0..packets {
            let last = i + 1 == packets;
            let granule = u64::from(pre_skip) + 960 * (i as u64 + 1);
            out.extend(page(
                if last { 0x04 } else { 0 },
                granule,
                i as u32 + 2,
                &[0xf8, i as u8, 0x55],
            ));
        }
        out
    }

    pub(crate) fn page(flags: u8, granule: u64, sequence: u32, body: &[u8]) -> Vec<u8> {
        let mut segments = vec![255u8; body.len() / 255];
        segments.push((body.len() % 255) as u8);
        let mut p = b"OggS".to_vec();
        p.push(0);
        p.push(flags);
        p.extend_from_slice(&granule.to_le_bytes());
        p.extend_from_slice(&0x6d6f_7373u32.to_le_bytes());
        p.extend_from_slice(&sequence.to_le_bytes());
        p.extend_from_slice(&[0; 4]);
        p.push(segments.len() as u8);
        p.extend_from_slice(&segments);
        p.extend_from_slice(body);
        let crc = ogg_crc(&p);
        p[22..26].copy_from_slice(&crc.to_le_bytes());
        p
    }

    #[test]
    fn length_comes_from_the_last_granule() {
        assert_eq!(inspect_ogg_opus(&ogg_opus(150)), Ok(3000));
        assert_eq!(inspect_ogg_opus(&ogg_opus(1)), Ok(20));
    }

    #[test]
    fn crc_matches_the_reference_check_value() {
        // RFC 3533's CRC is CRC-32/POSIX without the final xor: its check value
        // for "123456789" is 0x765e7680 ^ 0xffffffff.
        assert_eq!(ogg_crc(b""), 0);
        assert_eq!(ogg_crc(b"123456789"), 0x89a1_897f);
    }

    #[test]
    fn up_to_five_minutes_is_accepted() {
        assert_eq!(inspect_ogg_opus(&ogg_opus(15_000)), Ok(MAX_VOICE_MS));
        // Within the slack: clamped to the limit.
        assert_eq!(inspect_ogg_opus(&ogg_opus(15_040)), Ok(MAX_VOICE_MS));
        assert!(inspect_ogg_opus(&ogg_opus(15_100)).is_err());
    }

    #[test]
    fn rejects_damaged_or_foreign_files() {
        let good = ogg_opus(10);
        let invalid = Err(DomainError::InvalidField("voice"));
        assert_eq!(inspect_ogg_opus(&[]), invalid);
        assert_eq!(inspect_ogg_opus(b"\x1aE\xdf\xa3 webm"), invalid);
        // Any flipped byte breaks a page checksum or the structure.
        for i in [0, 5, 30, 60, good.len() - 1] {
            let mut bad = good.clone();
            bad[i] ^= 0x01;
            assert_eq!(inspect_ogg_opus(&bad), invalid, "byte {i}");
        }
        // Truncated: the last page is missing its end-of-stream flag.
        let cut = good.len() - page(0x04, 0, 11, &[0xf8, 9, 0x55]).len();
        assert_eq!(inspect_ogg_opus(&good[..cut]), invalid);
        // Trailing bytes after the end of the stream.
        let mut tail = good.clone();
        tail.extend_from_slice(&page(0, 0, 12, b"x"));
        assert_eq!(inspect_ogg_opus(&tail), invalid);
    }

    #[test]
    fn rejects_wrong_headers_and_order() {
        let invalid = Err(DomainError::InvalidField("voice"));
        // Pages out of order.
        let mut swapped = page(0x02, 0, 0, &ogg_opus_head());
        swapped.extend(page(0x04, 1272, 2, b"x"));
        assert_eq!(inspect_ogg_opus(&swapped), invalid);
        // A Vorbis or other stream instead of Opus.
        let mut vorbis = page(0x02, 0, 0, b"\x01vorbis\0\0\0\0\x01\x80\xbb\0\0");
        vorbis.extend(page(0, 0, 1, b"OpusTags"));
        vorbis.extend(page(0x04, 1272, 2, b"x"));
        assert_eq!(inspect_ogg_opus(&vorbis), invalid);
        // Granule smaller than the pre-skip.
        let mut short = page(0x02, 0, 0, &ogg_opus_head());
        short.extend(page(0, 0, 1, b"OpusTags"));
        short.extend(page(0x04, 100, 2, b"x"));
        assert_eq!(inspect_ogg_opus(&short), invalid);
    }

    #[test]
    fn rejects_files_over_the_byte_limit() {
        let mut big = ogg_opus(10);
        big.resize(MAX_VOICE_BYTES + 1, 0);
        assert!(inspect_ogg_opus(&big).is_err());
    }

    fn ogg_opus_head() -> Vec<u8> {
        let mut head = b"OpusHead".to_vec();
        head.extend_from_slice(&[1, 1]);
        head.extend_from_slice(&312u16.to_le_bytes());
        head.extend_from_slice(&48_000u32.to_le_bytes());
        head.extend_from_slice(&[0, 0, 0]);
        head
    }
}

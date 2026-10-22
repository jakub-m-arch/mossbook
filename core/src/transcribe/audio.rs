//! A recording turned into what the recognizer takes: 16 kHz mono samples,
//! cut into pieces short enough to keep memory and quality in check.

use core::ops::Range;

use opus::{Channels, Decoder};
use zeroize::{Zeroize, Zeroizing};

use super::TranscribeError;
use crate::domain::MAX_VOICE_MS;
use crate::domain::voice::opus_packets;

/// The rate the model expects; Opus decodes to it directly.
pub const SAMPLE_RATE: usize = 16_000;

/// Longest Opus frame (120 ms) at [`SAMPLE_RATE`], per channel.
const MAX_FRAME: usize = SAMPLE_RATE * 120 / 1000;
/// Decoded audio is cut off here: a little over the longest recording.
const MAX_SAMPLES: usize = (MAX_VOICE_MS as usize + 1000) * SAMPLE_RATE / 1000;

/// Pieces are cut between these lengths. In one pass, five minutes took 3.7 GB
/// of memory and came out worse than in pieces (spike for D-23).
const MIN_PIECE: usize = 15 * SAMPLE_RATE;
const MAX_PIECE: usize = 25 * SAMPLE_RATE;
/// Window in which loudness is compared to find a pause (100 ms).
const WINDOW: usize = SAMPLE_RATE / 10;

/// Decodes a stored Ogg Opus recording to 16 kHz mono. The samples are zeroed
/// on drop, like the recording itself.
pub fn decode(recording: &[u8]) -> Result<Zeroizing<Vec<f32>>, TranscribeError> {
    let stream = opus_packets(recording).map_err(|_| TranscribeError::Audio)?;
    let (channels, layout) = match stream.channels {
        1 => (1, Channels::Mono),
        _ => (2, Channels::Stereo),
    };
    let mut decoder =
        Decoder::new(SAMPLE_RATE as u32, layout).map_err(|_| TranscribeError::Audio)?;
    let mut frame = Zeroizing::new(vec![0f32; MAX_FRAME * channels]);
    let mut pcm = Zeroizing::new(Vec::with_capacity(MAX_SAMPLES));
    for packet in stream.packets {
        let n = decoder
            .decode_float(packet, &mut frame, false)
            .map_err(|_| TranscribeError::Audio)?;
        let decoded = frame.get(..n * channels).ok_or(TranscribeError::Audio)?;
        pcm.extend(
            decoded
                .chunks_exact(channels)
                .map(|c| c.iter().sum::<f32>() / channels as f32),
        );
        if pcm.len() > MAX_SAMPLES {
            pcm.truncate(MAX_SAMPLES);
            break;
        }
    }
    // The pre-skip counts 48 kHz samples (RFC 7845 §4.2).
    let skip = (usize::from(stream.pre_skip) / 3).min(pcm.len());
    pcm[..skip].zeroize();
    pcm.drain(..skip);
    Ok(pcm)
}

/// Cuts `pcm` into pieces of [`MIN_PIECE`]..[`MAX_PIECE`] samples, each ending
/// in the quietest 100 ms of that span, so a cut rarely falls inside a word.
pub fn pieces(pcm: &[f32]) -> Vec<Range<usize>> {
    let energy = |at: usize| pcm[at..at + WINDOW].iter().map(|x| x * x).sum::<f32>();
    let mut out = Vec::new();
    let mut start = 0;
    while pcm.len() - start > MAX_PIECE {
        let quietest = (start + MIN_PIECE..start + MAX_PIECE - WINDOW)
            .step_by(WINDOW / 2)
            .min_by(|&a, &b| energy(a).total_cmp(&energy(b)))
            .unwrap_or(start + MAX_PIECE - WINDOW);
        let cut = quietest + WINDOW / 2;
        out.push(start..cut);
        start = cut;
    }
    if start < pcm.len() {
        out.push(start..pcm.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_audio_is_one_piece() {
        assert!(pieces(&[]).is_empty());
        assert_eq!(pieces(&[0.1; 1000]), vec![0..1000]);
        assert_eq!(pieces(&vec![0.1; MAX_PIECE]), vec![0..MAX_PIECE]);
    }

    #[test]
    fn long_audio_is_cut_at_pauses() {
        // A steady tone with silences at 18 s and 41 s.
        let mut pcm = vec![0.3f32; 60 * SAMPLE_RATE];
        for at in [18, 41] {
            pcm[at * SAMPLE_RATE..at * SAMPLE_RATE + WINDOW * 2].fill(0.0);
        }
        let cuts = pieces(&pcm);
        assert_eq!(cuts.len(), 3, "{cuts:?}");
        assert_eq!(cuts[0].start, 0);
        assert_eq!(cuts[2].end, pcm.len());
        for pair in cuts.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "no gaps or overlaps");
        }
        for (cut, pause) in cuts.iter().zip([18, 41]) {
            let pause = pause * SAMPLE_RATE;
            assert!((pause..pause + WINDOW * 2).contains(&cut.end), "{cut:?}");
            assert!(pcm[cut.end] == 0.0);
        }
    }

    #[test]
    fn audio_without_pauses_is_still_cut_within_the_limits() {
        let pcm = vec![0.3f32; 5 * 60 * SAMPLE_RATE];
        let cuts = pieces(&pcm);
        assert_eq!(cuts.last().unwrap().end, pcm.len());
        for cut in &cuts[..cuts.len() - 1] {
            assert!((MIN_PIECE..=MAX_PIECE).contains(&cut.len()), "{cut:?}");
        }
    }

    #[test]
    fn the_container_is_checked_before_decoding() {
        assert_eq!(decode(b"").err(), Some(TranscribeError::Audio));
        assert_eq!(
            decode(b"OggS not really").err(),
            Some(TranscribeError::Audio)
        );
    }
}

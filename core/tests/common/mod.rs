//! Shared test helpers. Synthetic data only.

/// A synthetic Ogg Opus file of `packets` 20 ms packets with arbitrary bytes:
/// a valid container (RFC 3533, RFC 7845), not decodable audio. `seed`
/// changes the packet bytes so two recordings differ.
pub fn ogg_opus(packets: usize, seed: u8) -> Vec<u8> {
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
        let flags = if i + 1 == packets { 0x04 } else { 0 };
        let granule = u64::from(pre_skip) + 960 * (i as u64 + 1);
        let body = [0xf8, seed, i as u8, 0x55, 0xaa];
        out.extend(page(flags, granule, i as u32 + 2, &body));
    }
    out
}

fn page(flags: u8, granule: u64, sequence: u32, body: &[u8]) -> Vec<u8> {
    let mut p = b"OggS".to_vec();
    p.push(0);
    p.push(flags);
    p.extend_from_slice(&granule.to_le_bytes());
    p.extend_from_slice(&0x6d6f_7373u32.to_le_bytes());
    p.extend_from_slice(&sequence.to_le_bytes());
    p.extend_from_slice(&[0; 4]);
    p.push(1);
    p.push(body.len() as u8);
    p.extend_from_slice(body);
    let crc = crc(&p);
    p[22..26].copy_from_slice(&crc.to_le_bytes());
    p
}

/// RFC 3533 page checksum (CRC-32, polynomial 0x04c11db7, no reflection).
fn crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &b in data {
        crc ^= u32::from(b) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

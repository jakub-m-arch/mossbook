//! Authenticated encryption with XChaCha20-Poly1305 (SPEC §6.2).
//!
//! Blob format: `nonce (24 B, random) || ciphertext || tag (16 B)`.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use super::{CryptoError, Dek, KEY_LEN, VaultId, random_bytes};

/// XChaCha20 nonce length in bytes.
pub const NONCE_LEN: usize = 24;
/// Poly1305 tag length in bytes.
pub const TAG_LEN: usize = 16;
/// Blob size overhead over the plaintext.
pub const OVERHEAD: usize = NONCE_LEN + TAG_LEN;

const RECORD_AAD_PREFIX: &[u8] = b"mossbook/rec/v1";

/// Record type in the `records` and `media` tables (SPEC §7) and in the AAD (SPEC §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordType {
    Entry,
    Tag,
    Plan,
    Settings,
    /// A voice note recording (E-18). Lives in the `media` table, never in `records`.
    Voice,
    /// A deleted entry or tag (sync, D-22), at the deleted record's id.
    Tombstone,
}

impl RecordType {
    /// The types stored in the `records` table, ordered by code.
    pub const ALL: [Self; 5] = [
        Self::Entry,
        Self::Tag,
        Self::Plan,
        Self::Settings,
        Self::Tombstone,
    ];
    /// The types stored in the `media` table.
    pub const MEDIA: [Self; 1] = [Self::Voice];

    /// Code stored in the `records.type` column.
    pub const fn code(self) -> u8 {
        match self {
            Self::Entry => 1,
            Self::Tag => 2,
            Self::Plan => 3,
            Self::Settings => 4,
            Self::Voice => 5,
            Self::Tombstone => 6,
        }
    }

    /// Whether records of this type belong in the `media` table.
    pub const fn is_media(self) -> bool {
        matches!(self, Self::Voice)
    }

    /// Type from a `records.type` column code.
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Entry),
            2 => Some(Self::Tag),
            3 => Some(Self::Plan),
            4 => Some(Self::Settings),
            5 => Some(Self::Voice),
            6 => Some(Self::Tombstone),
            _ => None,
        }
    }

    /// ASCII label used in the AAD.
    pub const fn aad_label(self) -> &'static [u8] {
        match self {
            Self::Entry => b"entry",
            Self::Tag => b"tag",
            Self::Plan => b"plan",
            Self::Settings => b"settings",
            Self::Voice => b"voice",
            Self::Tombstone => b"tombstone",
        }
    }
}

/// Record AAD: `"mossbook/rec/v1" || vault_id || record_id || record_type`.
///
/// `vault_id` and `record_id` are fixed-length and the type label comes last,
/// so the concatenation is unambiguous.
pub fn record_aad(vault_id: &VaultId, record_id: &[u8; 16], record_type: RecordType) -> Vec<u8> {
    let label = record_type.aad_label();
    let mut aad = Vec::with_capacity(RECORD_AAD_PREFIX.len() + 32 + label.len());
    aad.extend_from_slice(RECORD_AAD_PREFIX);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(record_id);
    aad.extend_from_slice(label);
    aad
}

/// Encrypts `plaintext` under a random nonce.
pub(crate) fn seal(
    key: &[u8; KEY_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    random_bytes(&mut nonce)?;
    seal_with_nonce(key, &nonce, aad, plaintext)
}

/// Encrypts under the given nonce. Outside tests the nonce must be random (see [`seal`]).
pub(crate) fn seal_with_nonce(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::Internal)?;
    let ct = cipher
        .encrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::Internal)?;
    let mut blob = Vec::with_capacity(NONCE_LEN + ct.len());
    blob.extend_from_slice(nonce);
    blob.extend_from_slice(&ct);
    Ok(blob)
}

/// Decrypts a `nonce || ciphertext || tag` blob. The plaintext is zeroed on drop.
pub(crate) fn open(
    key: &[u8; KEY_LEN],
    aad: &[u8],
    blob: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if blob.len() < OVERHEAD {
        return Err(CryptoError::Malformed);
    }
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    let nonce = XNonce::try_from(nonce).map_err(|_| CryptoError::Malformed)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::Internal)?;
    cipher
        .decrypt(&nonce, Payload { msg: ct, aad })
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

/// Encrypts a record with the DEK, binding it to its place in the database via the AAD.
pub fn seal_record(
    dek: &Dek,
    vault_id: &VaultId,
    record_id: &[u8; 16],
    record_type: RecordType,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    seal(
        dek.expose(),
        &record_aad(vault_id, record_id, record_type),
        plaintext,
    )
}

/// Decrypts a record. Fails if the blob was tampered with or moved to another id/type/vault.
pub fn open_record(
    dek: &Dek,
    vault_id: &VaultId,
    record_id: &[u8; 16],
    record_type: RecordType,
    blob: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    open(
        dek.expose(),
        &record_aad(vault_id, record_id, record_type),
        blob,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const VAULT: VaultId = [0x11; 16];
    const REC_A: [u8; 16] = [0xaa; 16];
    const REC_B: [u8; 16] = [0xbb; 16];

    fn dek() -> Dek {
        Dek::from_bytes(&mut [0x42; KEY_LEN])
    }

    #[test]
    fn round_trip() {
        let dek = dek();
        let blob = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"hello").unwrap();
        assert_eq!(blob.len(), 5 + OVERHEAD);
        let pt = open_record(&dek, &VAULT, &REC_A, RecordType::Entry, &blob).unwrap();
        assert_eq!(pt.as_slice(), b"hello");
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let dek = dek();
        let blob = seal_record(&dek, &VAULT, &REC_A, RecordType::Plan, b"").unwrap();
        assert_eq!(blob.len(), OVERHEAD);
        let pt = open_record(&dek, &VAULT, &REC_A, RecordType::Plan, &blob).unwrap();
        assert!(pt.is_empty());
    }

    #[test]
    fn nonces_are_random() {
        let dek = dek();
        let a = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"x").unwrap();
        let b = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"x").unwrap();
        assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN]);
        assert_ne!(a, b);
    }

    #[test]
    fn wrong_key_fails() {
        let blob = seal_record(&dek(), &VAULT, &REC_A, RecordType::Entry, b"hello").unwrap();
        let other = Dek::from_bytes(&mut [0x43; KEY_LEN]);
        assert_eq!(
            open_record(&other, &VAULT, &REC_A, RecordType::Entry, &blob).unwrap_err(),
            CryptoError::Decrypt
        );
    }

    #[test]
    fn every_flipped_byte_is_detected() {
        let dek = dek();
        let blob = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"hello world").unwrap();
        for i in 0..blob.len() {
            let mut tampered = blob.clone();
            tampered[i] ^= 0x01;
            assert_eq!(
                open_record(&dek, &VAULT, &REC_A, RecordType::Entry, &tampered).unwrap_err(),
                CryptoError::Decrypt,
                "byte {i}"
            );
        }
    }

    #[test]
    fn truncated_blob_is_rejected() {
        let dek = dek();
        let blob = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"hello").unwrap();
        assert_eq!(
            open_record(
                &dek,
                &VAULT,
                &REC_A,
                RecordType::Entry,
                &blob[..OVERHEAD - 1]
            )
            .unwrap_err(),
            CryptoError::Malformed
        );
        assert_eq!(
            open_record(
                &dek,
                &VAULT,
                &REC_A,
                RecordType::Entry,
                &blob[..blob.len() - 1]
            )
            .unwrap_err(),
            CryptoError::Decrypt
        );
    }

    #[test]
    fn swapped_records_are_detected() {
        let dek = dek();
        let a = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"A").unwrap();
        let b = seal_record(&dek, &VAULT, &REC_B, RecordType::Entry, b"B").unwrap();
        assert!(open_record(&dek, &VAULT, &REC_A, RecordType::Entry, &b).is_err());
        assert!(open_record(&dek, &VAULT, &REC_B, RecordType::Entry, &a).is_err());
    }

    #[test]
    fn other_record_type_is_detected() {
        let dek = dek();
        let blob = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"A").unwrap();
        for rt in [
            RecordType::Tag,
            RecordType::Plan,
            RecordType::Settings,
            RecordType::Voice,
            RecordType::Tombstone,
        ] {
            assert!(open_record(&dek, &VAULT, &REC_A, rt, &blob).is_err());
        }
    }

    #[test]
    fn other_vault_is_detected() {
        let dek = dek();
        let blob = seal_record(&dek, &VAULT, &REC_A, RecordType::Entry, b"A").unwrap();
        assert!(open_record(&dek, &[0x12; 16], &REC_A, RecordType::Entry, &blob).is_err());
    }

    #[test]
    fn record_type_codes_round_trip() {
        for rt in RecordType::ALL.into_iter().chain(RecordType::MEDIA) {
            assert_eq!(RecordType::from_code(rt.code()), Some(rt));
        }
        assert!(RecordType::ALL.iter().all(|rt| !rt.is_media()));
        assert!(RecordType::MEDIA.iter().all(|rt| rt.is_media()));
        assert_eq!(RecordType::from_code(0), None);
        assert_eq!(RecordType::from_code(7), None);
    }

    #[test]
    fn record_aad_layout() {
        let aad = record_aad(&VAULT, &REC_A, RecordType::Settings);
        let mut expected = b"mossbook/rec/v1".to_vec();
        expected.extend_from_slice(&[0x11; 16]);
        expected.extend_from_slice(&[0xaa; 16]);
        expected.extend_from_slice(b"settings");
        assert_eq!(aad, expected);
    }

    /// Fixed blob-format vector. A different output means a format change (bump the version, see CLAUDE.md).
    #[test]
    fn fixed_vector() {
        let key = [0x42; KEY_LEN];
        let nonce: [u8; NONCE_LEN] = core::array::from_fn(|i| i as u8);
        let aad = record_aad(&VAULT, &REC_A, RecordType::Entry);
        let blob = seal_with_nonce(&key, &nonce, &aad, b"mossbook test vector").unwrap();
        assert_eq!(hex::encode(&blob), FIXED_VECTOR_HEX);
        assert_eq!(
            open(&key, &aad, &blob).unwrap().as_slice(),
            b"mossbook test vector"
        );
    }

    /// Cross-checked with libsodium / argon2-cffi / stdlib HKDF: `tools/verify-vectors.py`.
    const FIXED_VECTOR_HEX: &str = "000102030405060708090a0b0c0d0e0f1011121314151617e193f794a5cea8abd212c135761888dbb2ad64c8086ae00233da7c958f554aecd9ae8e2f";

    proptest! {
        #[test]
        fn prop_round_trip(pt in proptest::collection::vec(any::<u8>(), 0..4096),
                           id in any::<[u8; 16]>(), code in 1u8..=4) {
            let dek = dek();
            let rt = RecordType::from_code(code).unwrap();
            let blob = seal_record(&dek, &VAULT, &id, rt, &pt).unwrap();
            prop_assert_eq!(blob.len(), pt.len() + OVERHEAD);
            let back = open_record(&dek, &VAULT, &id, rt, &blob).unwrap();
            prop_assert_eq!(back.as_slice(), pt.as_slice());
        }
    }
}

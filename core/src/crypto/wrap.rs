//! Key wrapping: the DEK encrypted under a KEK (SPEC §6.1).
//!
//! XChaCha20-Poly1305 with a random nonce and `AAD = "mossbook/wrap/v1" || vault_id`.
//! Changing the password only re-wraps the DEK; records are never re-encrypted.
//! The device key (V-7) wraps the same DEK with its own AAD,
//! `"mossbook/wrap/device/v1" || vault_id`, so neither wrap can pose as the other.

use super::aead::{self, OVERHEAD};
use super::{CryptoError, Dek, DeviceKey, KEY_LEN, Kek, VaultId};

/// Length of a wrapped DEK in bytes.
pub const WRAPPED_LEN: usize = KEY_LEN + OVERHEAD;

const WRAP_AAD_PREFIX: &[u8] = b"mossbook/wrap/v1";
const DEVICE_WRAP_AAD_PREFIX: &[u8] = b"mossbook/wrap/device/v1";

fn wrap_aad(vault_id: &VaultId) -> Vec<u8> {
    [WRAP_AAD_PREFIX, vault_id.as_slice()].concat()
}

fn device_wrap_aad(vault_id: &VaultId) -> Vec<u8> {
    [DEVICE_WRAP_AAD_PREFIX, vault_id.as_slice()].concat()
}

/// Wraps `dek` under `kek`.
pub fn wrap_dek(kek: &Kek, vault_id: &VaultId, dek: &Dek) -> Result<Vec<u8>, CryptoError> {
    aead::seal(kek.expose(), &wrap_aad(vault_id), dek.expose())
}

/// Unwraps a DEK. A wrong password or recovery key surfaces as [`CryptoError::Decrypt`].
pub fn unwrap_dek(kek: &Kek, vault_id: &VaultId, wrapped: &[u8]) -> Result<Dek, CryptoError> {
    open_wrapped(kek.expose(), &wrap_aad(vault_id), wrapped)
}

/// Wraps `dek` under the device key (V-7).
pub fn wrap_dek_device(
    key: &DeviceKey,
    vault_id: &VaultId,
    dek: &Dek,
) -> Result<Vec<u8>, CryptoError> {
    aead::seal(key.expose(), &device_wrap_aad(vault_id), dek.expose())
}

/// Unwraps a DEK wrapped by [`wrap_dek_device`].
pub fn unwrap_dek_device(
    key: &DeviceKey,
    vault_id: &VaultId,
    wrapped: &[u8],
) -> Result<Dek, CryptoError> {
    open_wrapped(key.expose(), &device_wrap_aad(vault_id), wrapped)
}

fn open_wrapped(key: &[u8; KEY_LEN], aad: &[u8], wrapped: &[u8]) -> Result<Dek, CryptoError> {
    if wrapped.len() != WRAPPED_LEN {
        return Err(CryptoError::Malformed);
    }
    let plain = aead::open(key, aad, wrapped)?;
    let mut bytes: [u8; KEY_LEN] = plain
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Malformed)?;
    Ok(Dek::from_bytes(&mut bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::aead::{NONCE_LEN, RecordType, open_record, seal_record, seal_with_nonce};
    use crate::crypto::kdf::{KdfParams, derive_kek};
    use crate::crypto::recovery::RecoveryKey;

    const VAULT: VaultId = [0x11; 16];
    const FAST: KdfParams = KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    };
    const SALT: [u8; 16] = [0x5a; 16];

    fn kek(byte: u8) -> Kek {
        Kek::from_bytes(&mut [byte; KEY_LEN])
    }

    #[test]
    fn round_trip() {
        let dek = Dek::generate().unwrap();
        let wrapped = wrap_dek(&kek(1), &VAULT, &dek).unwrap();
        assert_eq!(wrapped.len(), WRAPPED_LEN);
        assert_eq!(
            unwrap_dek(&kek(1), &VAULT, &wrapped).unwrap().expose(),
            dek.expose()
        );
    }

    #[test]
    fn wrong_kek_or_vault_fails() {
        let wrapped = wrap_dek(&kek(1), &VAULT, &Dek::generate().unwrap()).unwrap();
        assert_eq!(
            unwrap_dek(&kek(2), &VAULT, &wrapped).unwrap_err(),
            CryptoError::Decrypt
        );
        assert_eq!(
            unwrap_dek(&kek(1), &[0x12; 16], &wrapped).unwrap_err(),
            CryptoError::Decrypt
        );
    }

    #[test]
    fn tampered_or_truncated_fails() {
        let wrapped = wrap_dek(&kek(1), &VAULT, &Dek::generate().unwrap()).unwrap();
        for i in 0..wrapped.len() {
            let mut t = wrapped.clone();
            t[i] ^= 0x80;
            assert_eq!(
                unwrap_dek(&kek(1), &VAULT, &t).unwrap_err(),
                CryptoError::Decrypt,
                "byte {i}"
            );
        }
        assert_eq!(
            unwrap_dek(&kek(1), &VAULT, &wrapped[1..]).unwrap_err(),
            CryptoError::Malformed
        );
    }

    #[test]
    fn record_blob_cannot_pose_as_wrapped_key() {
        // A record AAD differs from the wrap AAD even under the same key bytes.
        let dek = Dek::from_bytes(&mut [1u8; KEY_LEN]);
        let blob = seal_record(&dek, &VAULT, &[0; 16], RecordType::Entry, &[9u8; KEY_LEN]).unwrap();
        assert_eq!(
            unwrap_dek(&kek(1), &VAULT, &blob).unwrap_err(),
            CryptoError::Decrypt
        );
    }

    /// The whole hierarchy: password and recovery key both open the same DEK,
    /// and a password change (re-wrap) keeps existing records readable.
    #[test]
    fn password_and_recovery_open_same_dek_and_rekey_keeps_data() {
        let dek = Dek::generate().unwrap();
        let recovery = RecoveryKey::generate().unwrap();
        let wrapped_pwd = wrap_dek(
            &derive_kek("old password", &SALT, &FAST).unwrap(),
            &VAULT,
            &dek,
        )
        .unwrap();
        let wrapped_rec = wrap_dek(&recovery.derive_kek(), &VAULT, &dek).unwrap();
        let record = seal_record(&dek, &VAULT, &[7; 16], RecordType::Entry, b"note").unwrap();

        let via_pwd = unwrap_dek(
            &derive_kek("old password", &SALT, &FAST).unwrap(),
            &VAULT,
            &wrapped_pwd,
        )
        .unwrap();
        let via_rec = unwrap_dek(
            &RecoveryKey::from_phrase(&recovery.phrase())
                .unwrap()
                .derive_kek(),
            &VAULT,
            &wrapped_rec,
        )
        .unwrap();
        assert_eq!(via_pwd.expose(), dek.expose());
        assert_eq!(via_rec.expose(), dek.expose());

        // Password change: new salt, new KEK, same DEK.
        let new_salt = [0x77; 16];
        let rewrapped = wrap_dek(
            &derive_kek("new password", &new_salt, &FAST).unwrap(),
            &VAULT,
            &via_pwd,
        )
        .unwrap();
        assert!(
            unwrap_dek(
                &derive_kek("old password", &new_salt, &FAST).unwrap(),
                &VAULT,
                &rewrapped
            )
            .is_err()
        );
        let after = unwrap_dek(
            &derive_kek("new password", &new_salt, &FAST).unwrap(),
            &VAULT,
            &rewrapped,
        )
        .unwrap();
        let note = open_record(&after, &VAULT, &[7; 16], RecordType::Entry, &record).unwrap();
        assert_eq!(note.as_slice(), b"note");
    }

    fn device_key(byte: u8) -> DeviceKey {
        DeviceKey::from_bytes(&mut [byte; KEY_LEN])
    }

    #[test]
    fn device_wrap_round_trip_and_rejects_wrong_key_or_vault() {
        let dek = Dek::generate().unwrap();
        let wrapped = wrap_dek_device(&device_key(3), &VAULT, &dek).unwrap();
        assert_eq!(wrapped.len(), WRAPPED_LEN);
        assert_eq!(
            unwrap_dek_device(&device_key(3), &VAULT, &wrapped)
                .unwrap()
                .expose(),
            dek.expose()
        );
        assert_eq!(
            unwrap_dek_device(&device_key(4), &VAULT, &wrapped).unwrap_err(),
            CryptoError::Decrypt
        );
        assert_eq!(
            unwrap_dek_device(&device_key(3), &[0x12; 16], &wrapped).unwrap_err(),
            CryptoError::Decrypt
        );
        for i in 0..wrapped.len() {
            let mut t = wrapped.clone();
            t[i] ^= 0x01;
            assert!(
                unwrap_dek_device(&device_key(3), &VAULT, &t).is_err(),
                "byte {i}"
            );
        }
        assert_eq!(
            unwrap_dek_device(&device_key(3), &VAULT, &wrapped[..WRAPPED_LEN - 1]).unwrap_err(),
            CryptoError::Malformed
        );
    }

    /// Same key bytes, different AAD: a password wrap does not open as a device
    /// wrap, nor the other way round.
    #[test]
    fn device_and_password_wraps_are_domain_separated() {
        let dek = Dek::generate().unwrap();
        let by_password = wrap_dek(&kek(5), &VAULT, &dek).unwrap();
        let by_device = wrap_dek_device(&device_key(5), &VAULT, &dek).unwrap();
        assert_eq!(
            unwrap_dek_device(&device_key(5), &VAULT, &by_password).unwrap_err(),
            CryptoError::Decrypt
        );
        assert_eq!(
            unwrap_dek(&kek(5), &VAULT, &by_device).unwrap_err(),
            CryptoError::Decrypt
        );
    }

    /// Fixed vector for the device wrap format.
    #[test]
    fn device_fixed_vector() {
        let nonce: [u8; NONCE_LEN] = core::array::from_fn(|i| 0xd0 ^ i as u8);
        let wrapped = seal_with_nonce(
            &[0x03; KEY_LEN],
            &nonce,
            &device_wrap_aad(&VAULT),
            &[0x04; KEY_LEN],
        )
        .unwrap();
        assert_eq!(hex::encode(&wrapped), DEVICE_FIXED_VECTOR_HEX);
        let dek = unwrap_dek_device(&device_key(0x03), &VAULT, &wrapped).unwrap();
        assert_eq!(dek.expose(), &[0x04; KEY_LEN]);
    }

    /// Cross-checked with libsodium: `tools/verify-vectors.py`.
    const DEVICE_FIXED_VECTOR_HEX: &str = "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfc0c1c2c3c4c5c6c71faae23383679a0c7993606b600db610e4e9e900ada3c7529393d63e4425ce497da1e16c5c151b7ff0798c134400ecfc";

    /// Fixed vector for the wrap format.
    #[test]
    fn fixed_vector() {
        let nonce: [u8; NONCE_LEN] = core::array::from_fn(|i| 0xf0 ^ i as u8);
        let wrapped = seal_with_nonce(
            &[0x01; KEY_LEN],
            &nonce,
            &wrap_aad(&VAULT),
            &[0x02; KEY_LEN],
        )
        .unwrap();
        assert_eq!(hex::encode(&wrapped), FIXED_VECTOR_HEX);
        let dek = unwrap_dek(&kek(0x01), &VAULT, &wrapped).unwrap();
        assert_eq!(dek.expose(), &[0x02; KEY_LEN]);
    }

    /// Cross-checked with libsodium / argon2-cffi / stdlib HKDF: `tools/verify-vectors.py`.
    const FIXED_VECTOR_HEX: &str = "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeffe0e1e2e3e4e5e6e768707b08629c217797673da14b330b9c0d294353112bcebeccfce6c742cb59561ccc9ba21acb4137910f3c513ee5177b";
}

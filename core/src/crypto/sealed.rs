//! Inbox for quick entries written while the vault is locked (E-15, SPEC §6.4).
//!
//! The vault has an X25519 key pair. The public key is stored in plain text in
//! the header; the secret key is stored wrapped under the DEK. Anyone can
//! *append* a sealed box (libsodium `crypto_box_seal`: ephemeral X25519 +
//! XSalsa20-Poly1305) without the password, but only an unlocked vault can open it.

use crypto_box::aead::OsRng;
use crypto_box::{PublicKey, SecretKey};
use zeroize::{Zeroize, Zeroizing};

use super::aead;
use super::{CryptoError, Dek, KEY_LEN, VaultId};

/// X25519 public key length.
pub const PUBLIC_LEN: usize = 32;
/// Length of the wrapped secret key.
pub const WRAPPED_SECRET_LEN: usize = KEY_LEN + aead::OVERHEAD;

const WRAP_AAD_PREFIX: &[u8] = b"mossbook/inbox-key/v1";

fn wrap_aad(vault_id: &VaultId) -> Vec<u8> {
    [WRAP_AAD_PREFIX, vault_id.as_slice()].concat()
}

/// A new inbox key pair: the public key and the secret wrapped under `dek`.
pub fn generate(dek: &Dek, vault_id: &VaultId) -> Result<([u8; PUBLIC_LEN], Vec<u8>), CryptoError> {
    let secret = SecretKey::generate(&mut OsRng);
    let public = secret.public_key().to_bytes();
    let mut bytes = secret.to_bytes();
    let wrapped = aead::seal(dek.expose(), &wrap_aad(vault_id), &bytes);
    bytes.zeroize();
    Ok((public, wrapped?))
}

/// Seals `plaintext` to the vault's inbox public key. Needs no secret.
pub fn seal(public: &[u8; PUBLIC_LEN], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    PublicKey::from(*public)
        .seal(&mut OsRng, plaintext)
        .map_err(|_| CryptoError::Internal)
}

/// Opens a sealed box with the secret key unwrapped from the header.
pub fn open(
    dek: &Dek,
    vault_id: &VaultId,
    wrapped_secret: &[u8],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let raw = aead::open(dek.expose(), &wrap_aad(vault_id), wrapped_secret)?;
    let mut bytes: [u8; KEY_LEN] = raw
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Malformed)?;
    let secret = SecretKey::from_bytes(bytes);
    bytes.zeroize();
    secret
        .unseal(sealed)
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VAULT: VaultId = [0x11; 16];

    fn dek(byte: u8) -> Dek {
        Dek::from_bytes(&mut [byte; KEY_LEN])
    }

    #[test]
    fn seal_without_secret_open_with_dek() {
        let (public, wrapped) = generate(&dek(1), &VAULT).unwrap();
        assert_eq!(wrapped.len(), WRAPPED_SECRET_LEN);
        let sealed = seal(&public, b"quick entry").unwrap();
        assert_eq!(
            sealed.len(),
            b"quick entry".len() + 48,
            "ephemeral key + tag"
        );
        assert_eq!(
            open(&dek(1), &VAULT, &wrapped, &sealed).unwrap().as_slice(),
            b"quick entry"
        );
    }

    #[test]
    fn wrong_dek_vault_or_tampering_fails() {
        let (public, wrapped) = generate(&dek(1), &VAULT).unwrap();
        let sealed = seal(&public, b"x").unwrap();
        assert_eq!(
            open(&dek(2), &VAULT, &wrapped, &sealed).unwrap_err(),
            CryptoError::Decrypt
        );
        assert_eq!(
            open(&dek(1), &[0x12; 16], &wrapped, &sealed).unwrap_err(),
            CryptoError::Decrypt
        );
        for i in 0..sealed.len() {
            let mut t = sealed.clone();
            t[i] ^= 1;
            assert_eq!(
                open(&dek(1), &VAULT, &wrapped, &t).unwrap_err(),
                CryptoError::Decrypt,
                "byte {i}"
            );
        }
        // A box sealed to another vault's key does not open.
        let (other_public, _) = generate(&dek(1), &VAULT).unwrap();
        let foreign = seal(&other_public, b"x").unwrap();
        assert_eq!(
            open(&dek(1), &VAULT, &wrapped, &foreign).unwrap_err(),
            CryptoError::Decrypt
        );
    }

    /// A box sealed by libsodium (PyNaCl `SealedBox`, secret key 0x07 × 32) opens here.
    #[test]
    fn opens_a_libsodium_sealed_box() {
        const SEALED: &str = "377086537966e27e39ca3836f9981345673dfba79a50cf701ac9798bca8ecb742c0290eae8bd644da61443096fb3601041a565c3e5776f2de322638be5f47115fb8346d90a";
        let secret = SecretKey::from_bytes([7; 32]);
        let plain = secret.unseal(&hex::decode(SEALED).unwrap()).unwrap();
        assert_eq!(plain, b"mossbook inbox vector");
    }

    #[test]
    fn boxes_are_randomized() {
        let (public, _) = generate(&dek(1), &VAULT).unwrap();
        assert_ne!(
            seal(&public, b"same").unwrap(),
            seal(&public, b"same").unwrap()
        );
    }
}

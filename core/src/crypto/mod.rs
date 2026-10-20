//! Vault cryptography (SPEC §6).
//!
//! Composition of audited RustCrypto primitives only: XChaCha20-Poly1305,
//! Argon2id, HKDF-SHA256, BIP-39. No home-grown primitives.

pub mod aead;
pub mod kdf;
pub mod recovery;
pub mod sealed;
pub mod wrap;

use core::fmt;

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use zeroize::Zeroize;

/// Length of a symmetric key (DEK, KEK) in bytes.
pub const KEY_LEN: usize = 32;

/// Vault identifier (UUID, 16 bytes). Part of the AAD of every ciphertext.
pub type VaultId = [u8; 16];

/// Error of the crypto layer. Never carries any input data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// AEAD authentication failed: wrong key, tampered blob or different AAD.
    Decrypt,
    /// Blob is too short to contain a nonce and a tag.
    Malformed,
    /// KDF parameters out of the allowed range.
    InvalidKdfParams,
    /// Invalid recovery phrase (word count, unknown word, checksum).
    InvalidRecoveryPhrase,
    /// The system random number generator is unavailable.
    Rng,
    /// Internal primitive failure (e.g. Argon2 out of memory).
    Internal,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Decrypt => "decryption failed",
            Self::Malformed => "malformed ciphertext",
            Self::InvalidKdfParams => "invalid KDF parameters",
            Self::InvalidRecoveryPhrase => "invalid recovery phrase",
            Self::Rng => "system random number generator unavailable",
            Self::Internal => "internal cryptographic error",
        })
    }
}

impl std::error::Error for CryptoError {}

/// Fills `buf` with bytes from the operating system CSPRNG.
pub fn random_bytes(buf: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(buf).map_err(|_| CryptoError::Rng)
}

/// 256-bit key held in a `SecretBox` (zeroed on drop).
struct Key32(SecretBox<[u8; KEY_LEN]>);

impl Key32 {
    /// Copies the key into a `SecretBox` and zeroes the source buffer.
    fn from_bytes(bytes: &mut [u8; KEY_LEN]) -> Self {
        let mut key = SecretBox::new(Box::new([0u8; KEY_LEN]));
        key.expose_secret_mut().copy_from_slice(bytes);
        bytes.zeroize();
        Self(key)
    }

    fn generate() -> Result<Self, CryptoError> {
        let mut key = SecretBox::new(Box::new([0u8; KEY_LEN]));
        random_bytes(key.expose_secret_mut())?;
        Ok(Self(key))
    }

    fn expose(&self) -> &[u8; KEY_LEN] {
        self.0.expose_secret()
    }
}

macro_rules! secret_key_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        pub struct $name(Key32);

        impl $name {
            /// Exposes the raw key bytes. Only for handing to a primitive.
            pub(crate) fn expose(&self) -> &[u8; KEY_LEN] {
                self.0.expose()
            }

            /// Key from existing bytes (e.g. unwrapped key or test vector).
            /// The source buffer is zeroed.
            pub(crate) fn from_bytes(bytes: &mut [u8; KEY_LEN]) -> Self {
                Self(Key32::from_bytes(bytes))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    };
}

secret_key_type!(
    /// Data encryption key: random and fixed for the lifetime of the vault (SPEC §6.1).
    Dek
);

secret_key_type!(
    /// Key encryption key: derived from the password (Argon2id) or the recovery key (HKDF).
    Kek
);

impl Dek {
    /// Generates a new random DEK.
    pub fn generate() -> Result<Self, CryptoError> {
        Key32::generate().map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_keys() {
        let dek = Dek::from_bytes(&mut [7u8; KEY_LEN]);
        let kek = Kek::from_bytes(&mut [7u8; KEY_LEN]);
        assert_eq!(format!("{dek:?}"), "Dek([REDACTED])");
        assert_eq!(format!("{kek:?}"), "Kek([REDACTED])");
    }

    #[test]
    fn from_bytes_zeroes_source() {
        let mut src = [7u8; KEY_LEN];
        let dek = Dek::from_bytes(&mut src);
        assert_eq!(src, [0u8; KEY_LEN]);
        assert_eq!(dek.expose(), &[7u8; KEY_LEN]);
    }

    #[test]
    fn generated_deks_differ() {
        let a = Dek::generate().unwrap();
        let b = Dek::generate().unwrap();
        assert_ne!(a.expose(), b.expose());
    }
}

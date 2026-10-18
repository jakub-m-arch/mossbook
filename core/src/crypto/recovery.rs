//! Recovery key: 256 random bits shown to the user as 24 BIP-39 words (SPEC §6.1).
//!
//! `KEK_rec = HKDF-SHA256(ikm = entropy, salt = none, info = "mossbook/recovery/v1")`.
//! The entropy is uniformly random, so HKDF without a salt is sufficient.

use core::fmt;

use bip39::{Language, Mnemonic};
use hkdf::Hkdf;
use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

use super::{CryptoError, KEY_LEN, Kek, random_bytes};

/// Number of words in a recovery phrase.
pub const WORD_COUNT: usize = 24;

const HKDF_INFO: &[u8] = b"mossbook/recovery/v1";

/// Recovery key entropy, zeroed on drop.
pub struct RecoveryKey(SecretBox<[u8; KEY_LEN]>);

impl RecoveryKey {
    /// Generates a new random recovery key.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut entropy = SecretBox::new(Box::new([0u8; KEY_LEN]));
        random_bytes(entropy.expose_secret_mut())?;
        Ok(Self(entropy))
    }

    /// Parses a phrase typed by the user.
    ///
    /// Case and whitespace are normalized; the BIP-39 checksum catches most typos.
    pub fn from_phrase(phrase: &str) -> Result<Self, CryptoError> {
        let normalized: Zeroizing<String> = Zeroizing::new(
            phrase
                .split_whitespace()
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
                .join(" "),
        );
        if normalized.split(' ').count() != WORD_COUNT {
            return Err(CryptoError::InvalidRecoveryPhrase);
        }
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &normalized)
            .map_err(|_| CryptoError::InvalidRecoveryPhrase)?;
        let (mut array, len) = mnemonic.to_entropy_array();
        let result = if len == KEY_LEN {
            let mut entropy = SecretBox::new(Box::new([0u8; KEY_LEN]));
            entropy
                .expose_secret_mut()
                .copy_from_slice(&array[..KEY_LEN]);
            Ok(Self(entropy))
        } else {
            Err(CryptoError::InvalidRecoveryPhrase)
        };
        array.zeroize();
        result
    }

    fn mnemonic(&self) -> Mnemonic {
        // 32 bytes is always a valid BIP-39 entropy length.
        Mnemonic::from_entropy(self.0.expose_secret()).expect("32-byte entropy is valid for BIP-39")
    }

    /// The 24-word phrase, words separated by single spaces.
    pub fn phrase(&self) -> Zeroizing<String> {
        Zeroizing::new(self.mnemonic().to_string())
    }

    /// The word at `index` (0-based), used when asking the user to confirm the phrase.
    pub fn word(&self, index: usize) -> Option<&'static str> {
        self.mnemonic().words().nth(index)
    }

    /// Checks the words the user retyped at the given positions (V-1).
    pub fn confirm(&self, answers: &[(usize, &str)]) -> bool {
        !answers.is_empty()
            && answers.iter().all(|&(i, typed)| {
                self.word(i)
                    .is_some_and(|w| w == typed.trim().to_lowercase())
            })
    }

    /// Derives `KEK_rec`.
    pub fn derive_kek(&self) -> Kek {
        let mut okm = [0u8; KEY_LEN];
        Hkdf::<Sha256>::new(None, self.0.expose_secret())
            .expand(HKDF_INFO, &mut okm)
            .expect("32 bytes is a valid HKDF-SHA256 output length");
        Kek::from_bytes(&mut okm)
    }

    #[cfg(test)]
    fn from_entropy(mut entropy: [u8; KEY_LEN]) -> Self {
        let key = SecretBox::new(Box::new(entropy));
        entropy.zeroize();
        Self(key)
    }
}

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryKey([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BIP-39 reference vector (trezor/python-mnemonic vectors.json), 256-bit all-zero entropy.
    const ZERO_PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon \
        abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
        abandon abandon abandon abandon art";

    #[test]
    fn matches_bip39_reference_vector() {
        let key = RecoveryKey::from_entropy([0u8; KEY_LEN]);
        let expected = ZERO_PHRASE.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(key.phrase().as_str(), expected);
    }

    #[test]
    fn phrase_round_trip() {
        let key = RecoveryKey::generate().unwrap();
        let phrase = key.phrase();
        assert_eq!(phrase.split(' ').count(), WORD_COUNT);
        let back = RecoveryKey::from_phrase(&phrase).unwrap();
        assert_eq!(key.0.expose_secret(), back.0.expose_secret());
    }

    #[test]
    fn input_is_normalized() {
        let messy = format!("  {}\n", ZERO_PHRASE.to_uppercase().replace(' ', "   "));
        let key = RecoveryKey::from_phrase(&messy).unwrap();
        assert_eq!(key.0.expose_secret(), &[0u8; KEY_LEN]);
    }

    #[test]
    fn invalid_phrases_are_rejected() {
        let words: Vec<&str> = ZERO_PHRASE.split_whitespace().collect();
        // Wrong checksum word.
        let mut bad = words.clone();
        bad[23] = "abandon";
        // Word outside the list.
        let mut unknown = words.clone();
        unknown[3] = "mossbook";
        // 12-word phrase (valid BIP-39, but 128 bits).
        let twelve = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        for phrase in [
            bad.join(" "),
            unknown.join(" "),
            words[..23].join(" "),
            twelve.to_owned(),
            String::new(),
        ] {
            assert_eq!(
                RecoveryKey::from_phrase(&phrase).unwrap_err(),
                CryptoError::InvalidRecoveryPhrase
            );
        }
    }

    #[test]
    fn confirm_checks_given_positions() {
        let key = RecoveryKey::from_entropy([0u8; KEY_LEN]);
        assert!(key.confirm(&[(0, "abandon"), (23, " ART "), (11, "abandon")]));
        assert!(!key.confirm(&[(0, "abandon"), (23, "abandon")]));
        assert!(!key.confirm(&[(24, "abandon")]));
        assert!(!key.confirm(&[]));
    }

    #[test]
    fn different_keys_give_different_keks() {
        let a = RecoveryKey::from_entropy([0u8; KEY_LEN]).derive_kek();
        let b = RecoveryKey::from_entropy([1u8; KEY_LEN]).derive_kek();
        assert_ne!(a.expose(), b.expose());
    }

    #[test]
    fn debug_is_redacted() {
        assert_eq!(
            format!("{:?}", RecoveryKey::from_entropy([0u8; KEY_LEN])),
            "RecoveryKey([REDACTED])"
        );
    }

    /// Fixed vector: HKDF-SHA256(entropy = 0x00 * 32, info = "mossbook/recovery/v1").
    #[test]
    fn fixed_vector() {
        let kek = RecoveryKey::from_entropy([0u8; KEY_LEN]).derive_kek();
        assert_eq!(hex::encode(kek.expose()), FIXED_VECTOR_HEX);
    }

    /// Cross-checked with libsodium / argon2-cffi / stdlib HKDF: `tools/verify-vectors.py`.
    const FIXED_VECTOR_HEX: &str =
        "a3f3d01aa17e8f009d0eee4c9d5f7293c9d4ad20b01e42b5c5e121453ff91968";
}

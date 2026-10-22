//! Password change by re-wrapping the DEK (V-5). Records are not re-encrypted.

use crate::crypto::CryptoError;
use crate::crypto::kdf::{derive_kek, generate_salt};
use crate::crypto::recovery::RecoveryKey;
use crate::crypto::wrap::{unwrap_dek, wrap_dek};

use super::unlock::unwrap_with_password;
use super::{UnlockedVault, VaultError, VaultHeader, validate_new_password};

/// Checks `password` against the header without changing anything.
pub(super) fn verify_password(header: &VaultHeader, password: &str) -> Result<(), VaultError> {
    unwrap_with_password(header, password).map(drop)
}

impl UnlockedVault {
    /// Checks that `phrase` is this vault's recovery key, without changing
    /// anything. Lets the user test a written-down key before they need it.
    pub fn verify_recovery(&self, phrase: &str) -> Result<(), VaultError> {
        let recovery = RecoveryKey::from_phrase(phrase)?;
        unwrap_dek(
            &recovery.derive_kek(),
            &self.header.vault_id,
            &self.header.wrapped_dek_rec,
        )
        .map(drop)
        .map_err(|e| match e {
            CryptoError::Decrypt => VaultError::WrongRecoveryKey,
            CryptoError::Malformed => VaultError::Corrupted,
            other => other.into(),
        })
    }

    /// Changes the password after verifying the current one.
    pub fn change_password(&mut self, old: &str, new: &str) -> Result<(), VaultError> {
        validate_new_password(new)?;
        verify_password(&self.header, old)?;
        self.set_password(new)
    }

    /// Takes over a newer password wrap of the same journal (sync, D-22).
    pub(crate) fn adopt_password_wrap(&mut self, other: &VaultHeader) -> Result<bool, VaultError> {
        if other.vault_id != self.header.vault_id
            || other.password_epoch <= self.header.password_epoch
        {
            return Ok(false);
        }
        let mut header = self.header.clone();
        header.kdf_params = other.kdf_params;
        header.kdf_salt = other.kdf_salt;
        header.wrapped_dek_pwd = other.wrapped_dek_pwd.clone();
        header.password_epoch = other.password_epoch;
        header.save_password_wrap(&mut self.store)?;
        self.header = header;
        Ok(true)
    }

    /// Wraps the DEK under a KEK derived from `new` with a fresh salt.
    /// The KDF params of the vault are kept.
    pub(super) fn set_password(&mut self, new: &str) -> Result<(), VaultError> {
        let mut header = self.header.clone();
        header.kdf_salt = generate_salt()?;
        let kek = derive_kek(new, &header.kdf_salt, &header.kdf_params)?;
        header.wrapped_dek_pwd = wrap_dek(&kek, &header.vault_id, &self.dek)?;
        header.password_epoch = header.password_epoch.saturating_add(1);
        header.save_password_wrap(&mut self.store)?;
        self.header = header;
        Ok(())
    }
}

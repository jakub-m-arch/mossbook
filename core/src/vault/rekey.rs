//! Password change by re-wrapping the DEK (V-5). Records are not re-encrypted.

use crate::crypto::kdf::{derive_kek, generate_salt};
use crate::crypto::wrap::wrap_dek;

use super::unlock::unwrap_with_password;
use super::{UnlockedVault, VaultError, VaultHeader, validate_new_password};

/// Checks `password` against the header without changing anything.
pub(super) fn verify_password(header: &VaultHeader, password: &str) -> Result<(), VaultError> {
    unwrap_with_password(header, password).map(drop)
}

impl UnlockedVault {
    /// Changes the password after verifying the current one.
    pub fn change_password(&mut self, old: &str, new: &str) -> Result<(), VaultError> {
        validate_new_password(new)?;
        verify_password(&self.header, old)?;
        self.set_password(new)
    }

    /// Wraps the DEK under a KEK derived from `new` with a fresh salt.
    /// The KDF params of the vault are kept.
    pub(super) fn set_password(&mut self, new: &str) -> Result<(), VaultError> {
        let mut header = self.header.clone();
        header.kdf_salt = generate_salt()?;
        let kek = derive_kek(new, &header.kdf_salt, &header.kdf_params)?;
        header.wrapped_dek_pwd = wrap_dek(&kek, &header.vault_id, &self.dek)?;
        header.save_password_wrap(&mut self.store)?;
        self.header = header;
        Ok(())
    }
}

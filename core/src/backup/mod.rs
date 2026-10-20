//! Encrypted backups (X-1, X-2). Format: [`format`] and `docs/backup-format.md`.
//!
//! - **Export** copies the encrypted records and the vault header as they are:
//!   the backup opens with the password or recovery key valid *at export time*.
//! - **Restore** (no vault yet) turns a backup back into a vault file, unchanged.
//! - **Import** into an existing journal decrypts the backup with its own key and
//!   re-encrypts with the current vault's key (decision D-4); merge rules live in
//!   [`crate::journal::Journal::import`].

pub mod auto;
pub mod format;

use core::fmt;
use std::path::Path;

use crate::crypto::aead::{self, RecordType, open_record};
use crate::crypto::recovery::RecoveryKey;
use crate::crypto::wrap::unwrap_dek;
use crate::crypto::{CryptoError, Dek};
use crate::store::{RawRecord, Store};
use crate::vault::create::write_new_vault;
use crate::vault::unlock::{load, unwrap_with_password};
use crate::vault::{LoadedRecord, UnlockedVault, VaultError, VaultHeader};

pub use format::{BACKUP_VERSION, Backup, MAGIC, MAX_BACKUP_BYTES};
use format::{manifest_aad, records_digest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupError {
    /// The file does not start with the Mossbook signature.
    NotABackup,
    /// Written by a newer (or unknown) format version.
    UnsupportedVersion,
    /// Structure invalid, a record or the manifest does not authenticate,
    /// or records were added, removed or reordered.
    Corrupted,
    TooLarge,
    WrongPassword,
    WrongRecoveryKey,
    InvalidRecoveryPhrase,
    Vault(VaultError),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotABackup => f.write_str("not a Mossbook backup"),
            Self::UnsupportedVersion => f.write_str("unsupported backup version"),
            Self::Corrupted => f.write_str("backup is damaged"),
            Self::TooLarge => f.write_str("backup is too large"),
            Self::WrongPassword => f.write_str("wrong password for this backup"),
            Self::WrongRecoveryKey => f.write_str("recovery phrase does not match this backup"),
            Self::InvalidRecoveryPhrase => f.write_str("invalid recovery phrase"),
            Self::Vault(e) => fmt::Display::fmt(e, f),
        }
    }
}

impl std::error::Error for BackupError {}

impl From<VaultError> for BackupError {
    fn from(e: VaultError) -> Self {
        match e {
            VaultError::WrongPassword => Self::WrongPassword,
            VaultError::WrongRecoveryKey => Self::WrongRecoveryKey,
            VaultError::InvalidRecoveryPhrase => Self::InvalidRecoveryPhrase,
            VaultError::Corrupted => Self::Corrupted,
            other => Self::Vault(other),
        }
    }
}

impl From<CryptoError> for BackupError {
    fn from(e: CryptoError) -> Self {
        match e {
            CryptoError::InvalidRecoveryPhrase => Self::InvalidRecoveryPhrase,
            CryptoError::Decrypt | CryptoError::Malformed => Self::Corrupted,
            other => Self::Vault(VaultError::Crypto(other)),
        }
    }
}

impl UnlockedVault {
    /// Builds a backup of the whole vault as it is on disk.
    pub fn export_backup(&self) -> Result<Backup, BackupError> {
        let records = self.raw_records()?;
        let mut header = self.header().clone();
        // The inbox key pair is not part of backups (see format.rs).
        header.inbox_key = None;
        let manifest = aead::seal(
            self.dek().expose(),
            &manifest_aad(&header.vault_id),
            &records_digest(&records),
        )?;
        Ok(Backup {
            header,
            records,
            manifest,
        })
    }
}

/// A backup whose key is known and whose contents were verified.
pub struct OpenedBackup {
    header: VaultHeader,
    dek: Dek,
    records: Vec<RawRecord>,
}

impl fmt::Debug for OpenedBackup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedBackup")
            .field("records", &self.records.len())
            .finish_non_exhaustive()
    }
}

impl Backup {
    /// Opens the backup with the password that was valid when it was made.
    pub fn open_with_password(self, password: &str) -> Result<OpenedBackup, BackupError> {
        let dek = unwrap_with_password(&self.header, password)?;
        self.verified(dek)
    }

    /// Opens the backup with the vault's recovery phrase.
    pub fn open_with_recovery(self, phrase: &str) -> Result<OpenedBackup, BackupError> {
        let recovery = RecoveryKey::from_phrase(phrase)?;
        let dek = unwrap_dek(
            &recovery.derive_kek(),
            &self.header.vault_id,
            &self.header.wrapped_dek_rec,
        )
        .map_err(|e| match e {
            CryptoError::Decrypt => BackupError::WrongRecoveryKey,
            other => other.into(),
        })?;
        self.verified(dek)
    }

    /// Checks the manifest and every record before anything is used.
    fn verified(self, dek: Dek) -> Result<OpenedBackup, BackupError> {
        let digest = aead::open(
            dek.expose(),
            &manifest_aad(&self.header.vault_id),
            &self.manifest,
        )?;
        if digest.as_slice() != records_digest(&self.records) {
            return Err(BackupError::Corrupted);
        }
        let opened = OpenedBackup {
            header: self.header,
            dek,
            records: self.records,
        };
        opened.decrypt()?;
        Ok(opened)
    }
}

impl OpenedBackup {
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Decrypts every record with the backup's own key.
    pub fn decrypt(&self) -> Result<Vec<LoadedRecord>, BackupError> {
        self.records
            .iter()
            .map(|r| {
                let record_type =
                    RecordType::from_code(r.type_code).ok_or(BackupError::Corrupted)?;
                let plaintext = open_record(
                    &self.dek,
                    &self.header.vault_id,
                    &r.id,
                    record_type,
                    &r.blob,
                )?;
                Ok(LoadedRecord {
                    id: r.id,
                    record_type,
                    plaintext,
                })
            })
            .collect()
    }

    /// Restores the backup as a new vault at `path` (no vault there yet). The
    /// vault keeps the backup's password, recovery key and identity.
    pub fn restore(self, path: &Path) -> Result<UnlockedVault, BackupError> {
        write_new_vault(path, &self.header, &self.records)?;
        let store = Store::open(path).map_err(VaultError::from)?;
        Ok(load(path, store, self.header, self.dek)?)
    }
}

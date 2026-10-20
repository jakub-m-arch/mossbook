//! The vault: header, key hierarchy and decrypted records in memory (SPEC §6, §7, §8.3).
//!
//! Lifecycle: [`PendingVault`] (created in memory, nothing on disk until the
//! recovery phrase is confirmed) → [`UnlockedVault`] (DEK and plaintext in
//! memory) → dropped or [`UnlockedVault::lock`]ed (all secrets zeroed).

pub(crate) mod create;
mod header;
mod rekey;
mod throttle;
pub(crate) mod unlock;

use core::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::crypto::aead::{RecordType, seal_record};
use crate::crypto::{CryptoError, Dek, VaultId};
use crate::store::{RawRecord, Store, StoreError};

pub use create::PendingVault;
pub use header::{FORMAT_VERSION, VaultHeader};
pub use throttle::UnlockThrottle;
pub use unlock::{append_to_inbox, unlock, unlock_with_recovery};

/// Minimum password length in characters (V-1).
pub const MIN_PASSWORD_CHARS: usize = 8;

/// Record identifier (UUID, 16 bytes).
pub type RecordId = [u8; 16];

/// Vault error. Carries no user data, paths or key material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    /// No vault file at the given path.
    NotFound,
    /// A vault file already exists at the given path.
    AlreadyExists,
    /// The password does not open the vault.
    WrongPassword,
    /// The recovery phrase is well-formed but belongs to another vault.
    WrongRecoveryKey,
    /// The recovery phrase is not a valid 24-word phrase.
    InvalidRecoveryPhrase,
    /// The new password is shorter than [`MIN_PASSWORD_CHARS`].
    PasswordTooShort,
    /// The recovery phrase has not been confirmed yet (V-1).
    NotConfirmed,
    /// The vault was written in a format this build cannot read.
    UnsupportedFormat,
    /// The vault header is missing or damaged.
    Corrupted,
    /// Quick entries while locked need the inbox key, which a vault gets on its
    /// first unlock after an upgrade.
    InboxUnavailable,
    Crypto(CryptoError),
    Storage(StoreError),
    /// File system error (creating, renaming or removing the vault file).
    Io,
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("vault not found"),
            Self::AlreadyExists => f.write_str("vault already exists"),
            Self::WrongPassword => f.write_str("wrong password"),
            Self::WrongRecoveryKey => f.write_str("recovery phrase does not match this vault"),
            Self::InvalidRecoveryPhrase => f.write_str("invalid recovery phrase"),
            Self::PasswordTooShort => f.write_str("password is too short"),
            Self::NotConfirmed => f.write_str("recovery phrase not confirmed"),
            Self::UnsupportedFormat => f.write_str("unsupported vault format"),
            Self::Corrupted => f.write_str("vault file is damaged"),
            Self::InboxUnavailable => f.write_str("quick entry while locked is not available yet"),
            Self::Crypto(e) => fmt::Display::fmt(e, f),
            Self::Storage(e) => fmt::Display::fmt(e, f),
            Self::Io => f.write_str("file system error"),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<CryptoError> for VaultError {
    fn from(e: CryptoError) -> Self {
        match e {
            CryptoError::InvalidRecoveryPhrase => Self::InvalidRecoveryPhrase,
            other => Self::Crypto(other),
        }
    }
}

impl From<StoreError> for VaultError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::AlreadyExists => Self::AlreadyExists,
            other => Self::Storage(other),
        }
    }
}

impl From<std::io::Error> for VaultError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

/// Failure of [`UnlockedVault::destroy`].
#[derive(Debug)]
pub enum DestroyError {
    /// Password check failed; nothing was deleted and the vault is still open.
    Rejected {
        vault: Box<UnlockedVault>,
        error: VaultError,
    },
    /// Deletion failed after the vault was closed.
    Failed(VaultError),
}

/// Whether a vault file exists at `path`.
pub fn exists(path: &Path) -> bool {
    path.is_file()
}

/// Checks the length rule for a new password.
pub fn validate_new_password(password: &str) -> Result<(), VaultError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(VaultError::PasswordTooShort);
    }
    Ok(())
}

/// A record decrypted on unlock. The plaintext is zeroed on drop.
pub struct LoadedRecord {
    pub id: RecordId,
    pub record_type: RecordType,
    pub plaintext: Zeroizing<Vec<u8>>,
}

impl fmt::Debug for LoadedRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedRecord")
            .field("record_type", &self.record_type)
            .field("len", &self.plaintext.len())
            .finish_non_exhaustive()
    }
}

/// An open vault: holds the DEK and encrypts/stores records.
///
/// Records decrypted on unlock are handed out once via [`Self::take_loaded`];
/// the vault keeps no plaintext afterwards, so the domain layer is the only
/// in-memory copy. Dropping the vault zeroes the DEK.
pub struct UnlockedVault {
    path: PathBuf,
    store: Store,
    header: VaultHeader,
    dek: Dek,
    loaded: Vec<LoadedRecord>,
    /// Quick entries written while locked, opened on unlock (E-15).
    inbox: Vec<InboxItem>,
    unreadable: usize,
}

/// A quick entry from the inbox, decrypted on unlock.
pub struct InboxItem {
    pub id: RecordId,
    pub plaintext: Zeroizing<Vec<u8>>,
}

impl fmt::Debug for UnlockedVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnlockedVault")
            .field("loaded", &self.loaded.len())
            .field("unreadable", &self.unreadable)
            .finish_non_exhaustive()
    }
}

impl UnlockedVault {
    pub fn vault_id(&self) -> &VaultId {
        &self.header.vault_id
    }

    /// Number of records that failed to decrypt or had an unknown type on unlock.
    /// They stay untouched on disk; the UI can tell the user some data is damaged.
    pub fn unreadable_records(&self) -> usize {
        self.unreadable
    }

    pub(crate) fn header(&self) -> &VaultHeader {
        &self.header
    }

    pub(crate) fn dek(&self) -> &Dek {
        &self.dek
    }

    /// All records as stored (encrypted), e.g. for a backup.
    pub(crate) fn raw_records(&self) -> Result<Vec<RawRecord>, VaultError> {
        Ok(self.store.records_all()?.records)
    }

    /// Hands out the records decrypted on unlock. Returns an empty list on later calls.
    pub fn take_loaded(&mut self) -> Vec<LoadedRecord> {
        std::mem::take(&mut self.loaded)
    }

    /// Hands out the inbox items opened on unlock. Returns an empty list later.
    pub fn take_inbox(&mut self) -> Vec<InboxItem> {
        std::mem::take(&mut self.inbox)
    }

    /// Removes processed inbox rows (after their entries were stored).
    pub fn clear_inbox(&mut self, ids: &[RecordId]) -> Result<(), VaultError> {
        Ok(self.store.inbox_delete(ids)?)
    }

    /// Records the caller could not decode (e.g. invalid CBOR), so
    /// [`Self::unreadable_records`] reports them together with undecryptable ones.
    pub fn add_unreadable(&mut self, count: usize) {
        self.unreadable += count;
    }

    /// Encrypts and stores a record, replacing any record with the same id.
    pub fn put(
        &mut self,
        id: RecordId,
        record_type: RecordType,
        plaintext: &[u8],
    ) -> Result<(), VaultError> {
        let blob = seal_record(
            &self.dek,
            &self.header.vault_id,
            &id,
            record_type,
            plaintext,
        )?;
        self.store.record_put(&RawRecord {
            id,
            type_code: record_type.code(),
            blob,
        })?;
        Ok(())
    }

    /// Encrypts and stores `puts` and deletes `deletes` in one transaction.
    pub fn write_batch(
        &mut self,
        puts: &[(RecordId, RecordType, &[u8])],
        deletes: &[RecordId],
    ) -> Result<(), VaultError> {
        let raw = puts
            .iter()
            .map(|&(id, record_type, plaintext)| {
                let blob = seal_record(
                    &self.dek,
                    &self.header.vault_id,
                    &id,
                    record_type,
                    plaintext,
                )?;
                Ok(RawRecord {
                    id,
                    type_code: record_type.code(),
                    blob,
                })
            })
            .collect::<Result<Vec<_>, VaultError>>()?;
        Ok(self.store.apply(&raw, deletes)?)
    }

    /// Deletes a record. Returns whether it existed.
    pub fn delete(&mut self, id: &RecordId) -> Result<bool, VaultError> {
        let existed = self.store.record_delete(id)?;
        Ok(existed)
    }

    /// Compacts the file after mass deletion (SPEC §7).
    pub fn vacuum(&self) -> Result<(), VaultError> {
        Ok(self.store.vacuum()?)
    }

    /// Locks the vault: flushes the WAL, closes the file and zeroes all secrets.
    pub fn lock(self) -> Result<(), VaultError> {
        let Self { store, .. } = self;
        // DEK and plaintext are dropped (and zeroed) at the end of this scope.
        Ok(store.close()?)
    }

    /// Checks `password` without changing anything.
    pub fn verify_password(&self, password: &str) -> Result<(), VaultError> {
        rekey::verify_password(&self.header, password)
    }

    /// Permanently deletes the vault after re-checking the password (V-9).
    ///
    /// On a wrong password the vault is handed back unchanged, so the user stays
    /// unlocked. Removes the database and its WAL/SHM files. Deleted files are not
    /// overwritten (pointless on flash storage); their content is ciphertext anyway.
    pub fn destroy(self, password: &str) -> Result<(), DestroyError> {
        if let Err(error) = self.verify_password(password) {
            return Err(DestroyError::Rejected {
                vault: Box::new(self),
                error,
            });
        }
        let Self { path, store, .. } = self;
        store.close().map_err(|e| DestroyError::Failed(e.into()))?;
        for file in [path.clone(), sidecar(&path, "-wal"), sidecar(&path, "-shm")] {
            match fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(DestroyError::Failed(VaultError::Io));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// `vault.db` → `vault.db-wal` etc.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

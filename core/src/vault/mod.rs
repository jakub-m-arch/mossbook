//! The vault: header, key hierarchy and decrypted records in memory (SPEC §6, §7, §8.3).
//!
//! Lifecycle: [`PendingVault`] (created in memory, nothing on disk until the
//! recovery phrase is confirmed) → [`UnlockedVault`] (DEK and plaintext in
//! memory) → dropped or [`UnlockedVault::lock`]ed (all secrets zeroed).

mod create;
mod header;
mod rekey;
mod throttle;
mod unlock;

use core::fmt;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::crypto::aead::{RecordType, seal_record};
use crate::crypto::{CryptoError, Dek, VaultId};
use crate::store::{RawRecord, Store, StoreError};

pub use create::PendingVault;
pub use header::{FORMAT_VERSION, VaultHeader};
pub use throttle::UnlockThrottle;
pub use unlock::{unlock, unlock_with_recovery};

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

/// A decrypted record held in memory.
struct Record {
    record_type: RecordType,
    plaintext: Zeroizing<Vec<u8>>,
}

/// An open vault: the DEK and every decrypted record live in memory.
///
/// Dropping it zeroes the DEK and all plaintext. Records are opaque bytes here;
/// the domain layer (stage 4) serializes its types into them.
pub struct UnlockedVault {
    path: PathBuf,
    store: Store,
    header: VaultHeader,
    dek: Dek,
    records: HashMap<RecordId, Record>,
    unreadable: usize,
}

impl fmt::Debug for UnlockedVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnlockedVault")
            .field("records", &self.records.len())
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

    /// Plaintext of a record, if present.
    pub fn get(&self, id: &RecordId) -> Option<(RecordType, &[u8])> {
        self.records
            .get(id)
            .map(|r| (r.record_type, r.plaintext.as_slice()))
    }

    /// All records of the given type.
    pub fn iter(&self, record_type: RecordType) -> impl Iterator<Item = (&RecordId, &[u8])> {
        self.records
            .iter()
            .filter(move |(_, r)| r.record_type == record_type)
            .map(|(id, r)| (id, r.plaintext.as_slice()))
    }

    /// Encrypts and stores a record, replacing any record with the same id.
    pub fn put(
        &mut self,
        id: RecordId,
        record_type: RecordType,
        plaintext: Vec<u8>,
    ) -> Result<(), VaultError> {
        let plaintext = Zeroizing::new(plaintext);
        let blob = seal_record(
            &self.dek,
            &self.header.vault_id,
            &id,
            record_type,
            &plaintext,
        )?;
        self.store.record_put(&RawRecord {
            id,
            type_code: record_type.code(),
            blob,
        })?;
        self.records.insert(
            id,
            Record {
                record_type,
                plaintext,
            },
        );
        Ok(())
    }

    /// Deletes a record. Returns whether it existed.
    pub fn delete(&mut self, id: &RecordId) -> Result<bool, VaultError> {
        let existed = self.store.record_delete(id)?;
        self.records.remove(id);
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

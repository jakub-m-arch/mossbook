//! Settings that belong to this device, not to the journal (sync, D-22): which
//! device this is and where its sync folder is. Kept in `meta` in plain text
//! (the folder's location and a random id, no content), never in backups or
//! sync files, and readable while the journal is locked.

use std::path::Path;

use crate::store::Store;

use super::{UnlockedVault, VaultError, exists};

/// A device-local setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Local {
    /// Random id naming this device's sync file.
    DeviceId,
    /// Where the sync folder is: a path, or an Android document tree URI.
    SyncFolder,
}

impl Local {
    const fn key(self) -> &'static str {
        match self {
            Self::DeviceId => "device_id",
            Self::SyncFolder => "sync_folder",
        }
    }
}

impl UnlockedVault {
    pub fn local(&self, setting: Local) -> Result<Option<String>, VaultError> {
        text(self.store().meta_get(setting.key())?)
    }

    /// Sets (`Some`) or removes (`None`) a device-local setting.
    pub fn set_local(&mut self, setting: Local, value: Option<&str>) -> Result<(), VaultError> {
        match value {
            Some(v) => self
                .store_mut()
                .meta_set_all(&[(setting.key(), v.as_bytes())])?,
            None => self.store_mut().meta_delete(setting.key())?,
        }
        Ok(())
    }
}

/// Reads a device-local setting while the journal is locked.
pub fn read_local(path: &Path, setting: Local) -> Result<Option<String>, VaultError> {
    if !exists(path) {
        return Ok(None);
    }
    let store = Store::open(path)?;
    let value = text(store.meta_get(setting.key())?);
    store.close()?;
    value
}

fn text(bytes: Option<Vec<u8>>) -> Result<Option<String>, VaultError> {
    bytes
        .map(|b| String::from_utf8(b).map_err(|_| VaultError::Corrupted))
        .transpose()
}

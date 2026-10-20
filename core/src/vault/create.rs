//! Vault creation (V-1, decision D-3).
//!
//! The vault is built in memory first. Nothing touches the disk until the user
//! has retyped the requested recovery words, so a vault without a confirmed
//! recovery key can never exist on disk.

use core::fmt;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use zeroize::Zeroizing;

use crate::crypto::kdf::{CALIBRATION_TARGET, KdfParams, calibrate, derive_kek, generate_salt};
use crate::crypto::recovery::{RecoveryKey, WORD_COUNT};
use crate::crypto::sealed;
use crate::crypto::wrap::wrap_dek;
use crate::crypto::{Dek, VaultId, random_bytes};
use crate::store::{RawRecord, Store};

use super::header::InboxKey;
use super::{UnlockedVault, VaultError, VaultHeader, exists, sidecar, validate_new_password};

/// Number of words the user has to retype (V-1).
pub const CONFIRM_WORDS: usize = 3;

/// A vault that exists only in memory until its recovery phrase is confirmed.
pub struct PendingVault {
    header: VaultHeader,
    dek: Dek,
    recovery: RecoveryKey,
    positions: [usize; CONFIRM_WORDS],
    confirmed: bool,
}

impl fmt::Debug for PendingVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingVault")
            .field("confirmed", &self.confirmed)
            .finish_non_exhaustive()
    }
}

impl PendingVault {
    /// Prepares a new vault, calibrating Argon2id for this device (~0.75 s).
    pub fn new(password: &str) -> Result<Self, VaultError> {
        validate_new_password(password)?;
        Self::with_kdf_params(password, calibrate(CALIBRATION_TARGET)?)
    }

    /// Like [`Self::new`] with explicit KDF params. Intended for tests, which
    /// need cheap params; the app must use [`Self::new`].
    #[doc(hidden)]
    pub fn with_kdf_params(password: &str, kdf_params: KdfParams) -> Result<Self, VaultError> {
        validate_new_password(password)?;
        let vault_id = random_uuid_v4()?;
        let kdf_salt = generate_salt()?;
        let dek = Dek::generate()?;
        let recovery = RecoveryKey::generate()?;
        let kek_pwd = derive_kek(password, &kdf_salt, &kdf_params)?;
        let header = VaultHeader {
            vault_id,
            kdf_params,
            kdf_salt,
            wrapped_dek_pwd: wrap_dek(&kek_pwd, &vault_id, &dek)?,
            wrapped_dek_rec: wrap_dek(&recovery.derive_kek(), &vault_id, &dek)?,
            inbox_key: Some(new_inbox_key(&dek, &vault_id)?),
        };
        Ok(Self {
            header,
            dek,
            recovery,
            positions: random_positions()?,
            confirmed: false,
        })
    }

    /// The 24-word recovery phrase, to be shown once.
    pub fn recovery_phrase(&self) -> Zeroizing<String> {
        self.recovery.phrase()
    }

    /// The 0-based word positions the user must retype, in ascending order.
    pub fn confirmation_positions(&self) -> [usize; CONFIRM_WORDS] {
        self.positions
    }

    /// Checks the retyped words. `answers` must cover exactly the requested positions.
    pub fn confirm(&mut self, answers: &[(usize, &str)]) -> bool {
        let given: HashMap<usize, &str> = answers.iter().copied().collect();
        let covers_all = given.len() == answers.len()
            && given.len() == CONFIRM_WORDS
            && self.positions.iter().all(|p| given.contains_key(p));
        self.confirmed = covers_all && self.recovery.confirm(answers);
        self.confirmed
    }

    /// Writes the vault to `path` and returns it unlocked.
    ///
    /// The file is first written as `<path>.creating` and then renamed, so a crash
    /// never leaves a half-written vault at `path`.
    pub fn write(self, path: &Path) -> Result<UnlockedVault, VaultError> {
        if !self.confirmed {
            return Err(VaultError::NotConfirmed);
        }
        write_new_vault(path, &self.header, &[])?;
        let store = Store::open(path)?;
        let Self { header, dek, .. } = self;
        super::unlock::load(path, store, header, dek)
    }
}

/// Writes a new vault file with `header` and already-encrypted `records`.
///
/// The file is first written as `<path>.creating` and then renamed, so a crash
/// never leaves a half-written vault at `path`. Refuses to replace a vault.
pub(crate) fn write_new_vault(
    path: &Path,
    header: &VaultHeader,
    records: &[RawRecord],
) -> Result<(), VaultError> {
    if exists(path) {
        return Err(VaultError::AlreadyExists);
    }
    let tmp = sidecar(path, ".creating");
    for stale in [tmp.clone(), sidecar(&tmp, "-wal"), sidecar(&tmp, "-shm")] {
        match fs::remove_file(&stale) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(VaultError::Io),
            _ => {}
        }
    }
    let mut store = Store::create(&tmp)?;
    header.save(&mut store)?;
    store.apply(records, &[])?;
    store.close()?;
    if exists(path) {
        return Err(VaultError::AlreadyExists);
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// A fresh inbox key pair for quick entries while locked (E-15).
pub(crate) fn new_inbox_key(dek: &Dek, vault_id: &VaultId) -> Result<InboxKey, VaultError> {
    let (public, wrapped_secret) = sealed::generate(dek, vault_id)?;
    Ok(InboxKey {
        public,
        wrapped_secret,
    })
}

/// Random UUID v4 bytes for the vault id.
fn random_uuid_v4() -> Result<VaultId, VaultError> {
    let mut id = [0u8; 16];
    random_bytes(&mut id)?;
    id[6] = (id[6] & 0x0f) | 0x40;
    id[8] = (id[8] & 0x3f) | 0x80;
    Ok(id)
}

/// Three distinct random positions in `0..WORD_COUNT`, sorted.
fn random_positions() -> Result<[usize; CONFIRM_WORDS], VaultError> {
    let mut picked = HashSet::new();
    while picked.len() < CONFIRM_WORDS {
        let mut byte = [0u8; 1];
        random_bytes(&mut byte)?;
        // Rejection sampling: 240 = 10 * 24, so values below it are uniform mod 24.
        if byte[0] < 240 {
            picked.insert(usize::from(byte[0]) % WORD_COUNT);
        }
    }
    let mut positions: Vec<usize> = picked.into_iter().collect();
    positions.sort_unstable();
    Ok([positions[0], positions[1], positions[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_are_distinct_sorted_and_in_range() {
        for _ in 0..200 {
            let p = random_positions().unwrap();
            assert!(p[0] < p[1] && p[1] < p[2] && p[2] < WORD_COUNT, "{p:?}");
        }
    }

    #[test]
    fn vault_id_is_uuid_v4() {
        let id = random_uuid_v4().unwrap();
        assert_eq!(id[6] >> 4, 4);
        assert_eq!(id[8] >> 6, 0b10);
    }
}

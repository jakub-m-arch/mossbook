//! Unlocking with the password (V-2) or the recovery key (V-6).

use std::path::Path;

use crate::crypto::aead::{RecordType, open_record};
use crate::crypto::kdf::derive_kek;
use crate::crypto::recovery::RecoveryKey;
use crate::crypto::wrap::unwrap_dek;
use crate::crypto::{CryptoError, Dek, random_bytes, sealed};
use crate::store::Store;

use super::create::new_inbox_key;
use super::{
    InboxItem, LoadedRecord, RecordId, UnlockedVault, VaultError, VaultHeader, exists,
    validate_new_password,
};

/// Opens the vault with the password.
pub fn unlock(path: &Path, password: &str) -> Result<UnlockedVault, VaultError> {
    let store = open_existing(path)?;
    let header = VaultHeader::load(&store)?;
    let dek = unwrap_with_password(&header, password)?;
    load(path, store, header, dek)
}

/// Opens the vault with the recovery phrase and sets a new password.
/// The recovery key itself stays the same.
pub fn unlock_with_recovery(
    path: &Path,
    phrase: &str,
    new_password: &str,
) -> Result<UnlockedVault, VaultError> {
    validate_new_password(new_password)?;
    let recovery = RecoveryKey::from_phrase(phrase)?;
    let store = open_existing(path)?;
    let header = VaultHeader::load(&store)?;
    let dek = unwrap_dek(
        &recovery.derive_kek(),
        &header.vault_id,
        &header.wrapped_dek_rec,
    )
    .map_err(|e| match e {
        CryptoError::Decrypt => VaultError::WrongRecoveryKey,
        CryptoError::Malformed => VaultError::Corrupted,
        other => other.into(),
    })?;
    let mut vault = load(path, store, header, dek)?;
    vault.set_password(new_password)?;
    Ok(vault)
}

/// Appends a sealed quick entry while the vault is locked (E-15). Needs no
/// password: it can add to the vault but read nothing.
pub fn append_to_inbox(path: &Path, plaintext: &[u8]) -> Result<RecordId, VaultError> {
    let mut store = open_existing(path)?;
    let header = VaultHeader::load(&store)?;
    let key = header.inbox_key.ok_or(VaultError::InboxUnavailable)?;
    let sealed = sealed::seal(&key.public, plaintext)?;
    let mut id = [0u8; 16];
    random_bytes(&mut id)?;
    id[6] = (id[6] & 0x0f) | 0x40;
    id[8] = (id[8] & 0x3f) | 0x80;
    store.inbox_put(&id, &sealed)?;
    store.close()?;
    Ok(id)
}

/// Derives `KEK_pwd` and unwraps the DEK; a failed unwrap means a wrong password.
pub(crate) fn unwrap_with_password(
    header: &VaultHeader,
    password: &str,
) -> Result<Dek, VaultError> {
    let kek = derive_kek(password, &header.kdf_salt, &header.kdf_params)?;
    unwrap_dek(&kek, &header.vault_id, &header.wrapped_dek_pwd).map_err(|e| match e {
        CryptoError::Decrypt => VaultError::WrongPassword,
        CryptoError::Malformed => VaultError::Corrupted,
        other => other.into(),
    })
}

fn open_existing(path: &Path) -> Result<Store, VaultError> {
    if !exists(path) {
        return Err(VaultError::NotFound);
    }
    Ok(Store::open(path)?)
}

/// Decrypts every record into memory. Records that fail to decrypt or have an
/// unknown type are counted, not fatal: one damaged row must not lock the user
/// out of everything else.
pub(crate) fn load(
    path: &Path,
    mut store: Store,
    mut header: VaultHeader,
    dek: Dek,
) -> Result<UnlockedVault, VaultError> {
    // Vaults from before E-15 get their inbox key pair now.
    if header.inbox_key.is_none() {
        header.inbox_key = Some(new_inbox_key(&dek, &header.vault_id)?);
        header.save_inbox_key(&mut store)?;
    }
    let scan = store.records_all()?;
    let mut loaded = Vec::with_capacity(scan.records.len());
    let mut unreadable = scan.malformed;
    for raw in scan.records {
        // A media type in `records` is out of place: counted, never opened.
        let record_type = RecordType::from_code(raw.type_code).filter(|rt| !rt.is_media());
        let opened = record_type.map(|rt| {
            open_record(&dek, &header.vault_id, &raw.id, rt, &raw.blob).map(|pt| (rt, pt))
        });
        match opened {
            Some(Ok((record_type, plaintext))) => loaded.push(LoadedRecord {
                id: raw.id,
                record_type,
                plaintext,
            }),
            _ => unreadable += 1,
        }
    }
    let mut inbox = Vec::new();
    if let Some(key) = &header.inbox_key {
        for row in store.inbox_all()? {
            match sealed::open(&dek, &header.vault_id, &key.wrapped_secret, &row.blob) {
                Ok(plaintext) => inbox.push(InboxItem {
                    id: row.id,
                    plaintext,
                }),
                // Left in place and counted; never trusted.
                Err(_) => unreadable += 1,
            }
        }
    }
    Ok(UnlockedVault {
        path: path.to_path_buf(),
        store,
        header,
        dek,
        loaded,
        inbox,
        unreadable,
    })
}

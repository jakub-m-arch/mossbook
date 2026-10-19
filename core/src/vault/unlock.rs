//! Unlocking with the password (V-2) or the recovery key (V-6).

use std::collections::HashMap;
use std::path::Path;

use crate::crypto::aead::{RecordType, open_record};
use crate::crypto::kdf::derive_kek;
use crate::crypto::recovery::RecoveryKey;
use crate::crypto::wrap::unwrap_dek;
use crate::crypto::{CryptoError, Dek};
use crate::store::Store;

use super::{Record, UnlockedVault, VaultError, VaultHeader, exists, validate_new_password};

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

/// Derives `KEK_pwd` and unwraps the DEK; a failed unwrap means a wrong password.
pub(super) fn unwrap_with_password(
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
pub(super) fn load(
    path: &Path,
    store: Store,
    header: VaultHeader,
    dek: Dek,
) -> Result<UnlockedVault, VaultError> {
    let scan = store.records_all()?;
    let mut records = HashMap::with_capacity(scan.records.len());
    let mut unreadable = scan.malformed;
    for raw in scan.records {
        let opened = RecordType::from_code(raw.type_code).map(|rt| {
            open_record(&dek, &header.vault_id, &raw.id, rt, &raw.blob).map(|pt| (rt, pt))
        });
        match opened {
            Some(Ok((record_type, plaintext))) => {
                records.insert(
                    raw.id,
                    Record {
                        record_type,
                        plaintext,
                    },
                );
            }
            _ => unreadable += 1,
        }
    }
    Ok(UnlockedVault {
        path: path.to_path_buf(),
        store,
        header,
        dek,
        records,
        unreadable,
    })
}

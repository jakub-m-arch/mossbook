//! The `.mossbook` backup file (SPEC §7, X-1). Documented in `docs/backup-format.md`.
//!
//! ```text
//! "MOSSBOOK" (8 B) || format version (u16 LE) || CBOR body
//! ```
//!
//! The body carries the vault header and the encrypted records exactly as they
//! are stored in `vault.db`, plus a manifest: SHA-256 over all records, sealed
//! with the DEK. Records are authenticated one by one; the manifest also makes
//! removing, adding or reordering records detectable.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::crypto::VaultId;
use crate::crypto::aead::RecordType;
use crate::crypto::kdf::{KdfParams, SALT_LEN};
use crate::crypto::wrap::WRAPPED_LEN;
use crate::store::RawRecord;
use crate::vault::{FORMAT_VERSION as VAULT_FORMAT_VERSION, VaultHeader};

use super::BackupError;

/// File signature.
pub const MAGIC: &[u8; 8] = b"MOSSBOOK";
/// Backup format version written by this build.
pub const BACKUP_VERSION: u16 = 1;
/// Largest file accepted on import, as a guard against memory exhaustion.
pub const MAX_BACKUP_BYTES: usize = 512 * 1024 * 1024;

/// AAD prefix of the sealed manifest.
pub(super) const MANIFEST_AAD_PREFIX: &[u8] = b"mossbook/backup/v1";

#[derive(Serialize, Deserialize)]
struct Body {
    /// `format_version` of the vault the records come from.
    vault_format: u16,
    #[serde(with = "serde_bytes")]
    vault_id: Vec<u8>,
    #[serde(with = "serde_bytes")]
    kdf_params: Vec<u8>,
    #[serde(with = "serde_bytes")]
    kdf_salt: Vec<u8>,
    #[serde(with = "serde_bytes")]
    wrapped_dek_pwd: Vec<u8>,
    #[serde(with = "serde_bytes")]
    wrapped_dek_rec: Vec<u8>,
    records: Vec<BodyRecord>,
    /// `nonce || AEAD(DEK, manifest AAD, SHA-256 of the records)`.
    #[serde(with = "serde_bytes")]
    manifest: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct BodyRecord {
    #[serde(with = "serde_bytes")]
    id: Vec<u8>,
    #[serde(rename = "type")]
    type_code: u8,
    #[serde(with = "serde_bytes")]
    blob: Vec<u8>,
}

/// A parsed backup. Everything in it is public or ciphertext.
#[derive(Clone, PartialEq, Eq)]
pub struct Backup {
    pub header: VaultHeader,
    pub records: Vec<RawRecord>,
    pub(super) manifest: Vec<u8>,
}

impl core::fmt::Debug for Backup {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Backup")
            .field("records", &self.records.len())
            .finish_non_exhaustive()
    }
}

impl Backup {
    /// Serializes to the file format.
    pub fn to_bytes(&self) -> Result<Vec<u8>, BackupError> {
        let h = &self.header;
        let body = Body {
            vault_format: VAULT_FORMAT_VERSION,
            vault_id: h.vault_id.to_vec(),
            kdf_params: h.kdf_params.to_bytes().to_vec(),
            kdf_salt: h.kdf_salt.to_vec(),
            wrapped_dek_pwd: h.wrapped_dek_pwd.clone(),
            wrapped_dek_rec: h.wrapped_dek_rec.clone(),
            records: self
                .records
                .iter()
                .map(|r| BodyRecord {
                    id: r.id.to_vec(),
                    type_code: r.type_code,
                    blob: r.blob.clone(),
                })
                .collect(),
            manifest: self.manifest.clone(),
        };
        let mut out = Vec::with_capacity(
            16 + self
                .records
                .iter()
                .map(|r| r.blob.len() + 32)
                .sum::<usize>(),
        );
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&BACKUP_VERSION.to_le_bytes());
        ciborium::into_writer(&body, &mut out).map_err(|_| BackupError::Corrupted)?;
        Ok(out)
    }

    /// Parses and validates the structure (not the cryptography: that needs a key).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BackupError> {
        if bytes.len() > MAX_BACKUP_BYTES {
            return Err(BackupError::TooLarge);
        }
        let rest = bytes
            .strip_prefix(MAGIC.as_slice())
            .ok_or(BackupError::NotABackup)?;
        let (version, body) = rest.split_at_checked(2).ok_or(BackupError::Corrupted)?;
        if u16::from_le_bytes([version[0], version[1]]) != BACKUP_VERSION {
            return Err(BackupError::UnsupportedVersion);
        }
        let body: Body = ciborium::from_reader(body).map_err(|_| BackupError::Corrupted)?;
        if body.vault_format != VAULT_FORMAT_VERSION {
            return Err(BackupError::UnsupportedVersion);
        }
        let fixed = |v: &[u8]| -> Result<[u8; 16], BackupError> {
            v.try_into().map_err(|_| BackupError::Corrupted)
        };
        let header = VaultHeader {
            vault_id: fixed(&body.vault_id)?,
            kdf_params: KdfParams::from_bytes(&body.kdf_params)
                .map_err(|_| BackupError::Corrupted)?,
            kdf_salt: <[u8; SALT_LEN]>::try_from(body.kdf_salt.as_slice())
                .map_err(|_| BackupError::Corrupted)?,
            wrapped_dek_pwd: body.wrapped_dek_pwd,
            wrapped_dek_rec: body.wrapped_dek_rec,
            // Not part of backups; a restored vault gets a new pair on unlock.
            inbox_key: None,
        };
        if header.wrapped_dek_pwd.len() != WRAPPED_LEN
            || header.wrapped_dek_rec.len() != WRAPPED_LEN
        {
            return Err(BackupError::Corrupted);
        }
        let records = body
            .records
            .into_iter()
            .map(|r| {
                RecordType::from_code(r.type_code).ok_or(BackupError::Corrupted)?;
                Ok(RawRecord {
                    id: fixed(&r.id)?,
                    type_code: r.type_code,
                    blob: r.blob,
                })
            })
            .collect::<Result<Vec<_>, BackupError>>()?;
        Ok(Self {
            header,
            records,
            manifest: body.manifest,
        })
    }

    pub fn vault_id(&self) -> &VaultId {
        &self.header.vault_id
    }
}

/// SHA-256 over every record, length-prefixed so the encoding is unambiguous.
pub(super) fn records_digest(records: &[RawRecord]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((records.len() as u64).to_le_bytes());
    for r in records {
        hash.update(r.id);
        hash.update([r.type_code]);
        hash.update((r.blob.len() as u64).to_le_bytes());
        hash.update(&r.blob);
    }
    hash.finalize().into()
}

pub(super) fn manifest_aad(vault_id: &VaultId) -> Vec<u8> {
    [MANIFEST_AAD_PREFIX, vault_id.as_slice()].concat()
}

//! The `.mossbook` backup file (SPEC §7, X-1). Documented in `docs/backup-format.md`.
//!
//! ```text
//! "MOSSBOOK" (8 B) || format version (u16 LE) || CBOR body
//! ```
//!
//! The body carries the vault header, the encrypted records and (version 2)
//! the encrypted recordings exactly as they are stored in `vault.db`, plus a
//! manifest: SHA-256 over all of them, sealed with the DEK. Records are
//! authenticated one by one; the manifest also makes removing, adding or
//! reordering records or recordings detectable.

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
/// Backup format version written by this build. Versions 1 (no recordings) and
/// 2 (no tombstones) are still read.
pub const BACKUP_VERSION: u16 = 3;
/// Largest file accepted on import, as a guard against memory exhaustion.
pub const MAX_BACKUP_BYTES: usize = 512 * 1024 * 1024;

/// AAD prefix of the sealed manifest: `"mossbook/backup/v" || version`. A label
/// per version means a backup cannot be passed off as an older version with
/// its recordings or tombstones cut out.

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
    /// Password changes so far (version 3; sync, D-22).
    #[serde(default)]
    password_epoch: u32,
    records: Vec<BodyRecord>,
    /// Recordings from the `media` table (version 2; absent in version 1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    media: Vec<BodyRecord>,
    /// `nonce || AEAD(DEK, manifest AAD, SHA-256 of the records and recordings)`.
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
    /// Encrypted recordings (E-18); always empty in a version 1 backup.
    pub media: Vec<RawRecord>,
    /// The format version the file was read in (or is written in).
    pub(super) version: u16,
    pub(super) manifest: Vec<u8>,
}

impl core::fmt::Debug for Backup {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Backup")
            .field("records", &self.records.len())
            .field("media", &self.media.len())
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
            password_epoch: h.password_epoch,
            records: self.records.iter().map(BodyRecord::from).collect(),
            media: self.media.iter().map(BodyRecord::from).collect(),
            manifest: self.manifest.clone(),
        };
        let mut out = Vec::with_capacity(
            16 + self
                .records
                .iter()
                .chain(&self.media)
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
        let version = u16::from_le_bytes([version[0], version[1]]);
        // Each backup version comes from exactly one vault format.
        let vault_format = match version {
            1 | 2 => version,
            BACKUP_VERSION => VAULT_FORMAT_VERSION,
            _ => return Err(BackupError::UnsupportedVersion),
        };
        let body: Body = ciborium::from_reader(body).map_err(|_| BackupError::Corrupted)?;
        if body.vault_format != vault_format {
            return Err(BackupError::UnsupportedVersion);
        }
        if (version == 1 && !body.media.is_empty()) || (version < 3 && body.password_epoch != 0) {
            return Err(BackupError::Corrupted);
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
            password_epoch: body.password_epoch,
        };
        if header.wrapped_dek_pwd.len() != WRAPPED_LEN
            || header.wrapped_dek_rec.len() != WRAPPED_LEN
        {
            return Err(BackupError::Corrupted);
        }
        // Records hold only record types and recordings only media types;
        // tombstones exist from version 3.
        let rows = |rows: Vec<BodyRecord>, media: bool| {
            rows.into_iter()
                .map(|r| {
                    RecordType::from_code(r.type_code)
                        .filter(|rt| rt.is_media() == media)
                        .filter(|rt| version >= 3 || *rt != RecordType::Tombstone)
                        .ok_or(BackupError::Corrupted)?;
                    Ok(RawRecord {
                        id: fixed(&r.id)?,
                        type_code: r.type_code,
                        blob: r.blob,
                    })
                })
                .collect::<Result<Vec<_>, BackupError>>()
        };
        Ok(Self {
            header,
            records: rows(body.records, false)?,
            media: rows(body.media, true)?,
            version,
            manifest: body.manifest,
        })
    }

    pub fn vault_id(&self) -> &VaultId {
        &self.header.vault_id
    }

    /// What the manifest covers, unsealed: equal for two exports of an
    /// unchanged journal although their bytes differ (fresh nonce). Lets a
    /// device skip rewriting its sync file when nothing changed (D-22).
    pub fn fingerprint(&self) -> [u8; 32] {
        manifest_digest(self.version, &self.header, &self.records, &self.media)
    }
}

impl From<&RawRecord> for BodyRecord {
    fn from(r: &RawRecord) -> Self {
        Self {
            id: r.id.to_vec(),
            type_code: r.type_code,
            blob: r.blob.clone(),
        }
    }
}

/// SHA-256 over every record, from version 2 every recording, and from
/// version 3 first the header fields a device adopts from another's file (the
/// password wrap and its count). Each list and blob is length-prefixed, so the
/// encoding is unambiguous.
pub(super) fn manifest_digest(
    version: u16,
    header: &VaultHeader,
    records: &[RawRecord],
    media: &[RawRecord],
) -> [u8; 32] {
    fn list(hash: &mut Sha256, rows: &[RawRecord]) {
        hash.update((rows.len() as u64).to_le_bytes());
        for r in rows {
            hash.update(r.id);
            hash.update([r.type_code]);
            hash.update((r.blob.len() as u64).to_le_bytes());
            hash.update(&r.blob);
        }
    }
    let mut hash = Sha256::new();
    if version >= 3 {
        hash.update(header.password_epoch.to_le_bytes());
        hash.update(header.kdf_params.to_bytes());
        hash.update(header.kdf_salt);
        hash.update(&header.wrapped_dek_pwd);
        hash.update(&header.wrapped_dek_rec);
    }
    list(&mut hash, records);
    if version >= 2 {
        list(&mut hash, media);
    }
    hash.finalize().into()
}

pub(super) fn manifest_aad(version: u16, vault_id: &VaultId) -> Vec<u8> {
    [
        format!("mossbook/backup/v{version}").as_bytes(),
        vault_id.as_slice(),
    ]
    .concat()
}

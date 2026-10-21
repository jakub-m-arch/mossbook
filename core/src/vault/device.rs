//! Unlocking with a key held by the device: fingerprint unlock (V-7, SPEC §6.5, D-18).
//!
//! A random [`DeviceKey`] wraps the DEK. The device key itself is encrypted by the
//! platform's secure hardware (Android Keystore, released only after a strong
//! biometric check); this module only sees that ciphertext as opaque bytes. Both
//! live in a file next to the vault, `vault.db.device-key`, which never goes into
//! backups: it would be useless on another device.
//!
//! File format v1 (all integers big-endian):
//!
//! ```text
//! magic "MBDK" (4) | version 1 (1) | vault_id (16) | wrapped DEK (72)
//!                  | sealed key length (2) | sealed key (1..=1024)
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::crypto::wrap::{WRAPPED_LEN, unwrap_dek_device, wrap_dek_device};
use crate::crypto::{CryptoError, DeviceKey, KEY_LEN, VaultId};
use crate::store::Store;

use super::unlock::load;
use super::{UnlockedVault, VaultError, VaultHeader, exists, sidecar};

/// Version of the device key file format.
pub const DEVICE_KEY_FORMAT: u8 = 1;

const MAGIC: &[u8; 4] = b"MBDK";
/// Upper bound for the platform's ciphertext of the device key (an AES-GCM
/// IV, 32 bytes and a tag is about 60 bytes).
pub const MAX_SEALED_KEY_LEN: usize = 1024;
const HEADER_LEN: usize = MAGIC.len() + 1 + 16 + WRAPPED_LEN + 2;

/// The device key file: the DEK wrapped under the device key, and the device key
/// as encrypted by the platform.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceKeyFile {
    vault_id: VaultId,
    wrapped_dek: Vec<u8>,
    sealed_key: Vec<u8>,
}

impl std::fmt::Debug for DeviceKeyFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceKeyFile([REDACTED])")
    }
}

impl DeviceKeyFile {
    /// Where the device key file of the vault at `vault` lives.
    pub fn path_for(vault: &Path) -> PathBuf {
        sidecar(vault, ".device-key")
    }

    /// The device key as encrypted by the platform, to hand back to it for opening.
    pub fn sealed_key(&self) -> &[u8] {
        &self.sealed_key
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.sealed_key.len());
        out.extend_from_slice(MAGIC);
        out.push(DEVICE_KEY_FORMAT);
        out.extend_from_slice(&self.vault_id);
        out.extend_from_slice(&self.wrapped_dek);
        // The length is checked on construction, so it fits in two bytes.
        out.extend_from_slice(&(self.sealed_key.len() as u16).to_be_bytes());
        out.extend_from_slice(&self.sealed_key);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, VaultError> {
        if bytes.len() < HEADER_LEN || &bytes[..4] != MAGIC {
            return Err(VaultError::Corrupted);
        }
        if bytes[4] != DEVICE_KEY_FORMAT {
            return Err(VaultError::UnsupportedFormat);
        }
        let vault_id: VaultId = bytes[5..21].try_into().map_err(|_| VaultError::Corrupted)?;
        let wrapped_dek = bytes[21..21 + WRAPPED_LEN].to_vec();
        let len_at = 21 + WRAPPED_LEN;
        let sealed_len = u16::from_be_bytes([bytes[len_at], bytes[len_at + 1]]) as usize;
        let sealed_key = &bytes[HEADER_LEN..];
        if sealed_len == 0 || sealed_len > MAX_SEALED_KEY_LEN || sealed_key.len() != sealed_len {
            return Err(VaultError::Corrupted);
        }
        Ok(Self {
            vault_id,
            wrapped_dek,
            sealed_key: sealed_key.to_vec(),
        })
    }

    /// Reads the device key file of the vault at `vault`; `None` if there is none.
    pub fn read(vault: &Path) -> Result<Option<Self>, VaultError> {
        match fs::read(Self::path_for(vault)) {
            Ok(bytes) => Self::from_bytes(&bytes).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(VaultError::Io),
        }
    }

    /// Writes the file next to the vault at `vault`, replacing an older one
    /// atomically (temporary file, then rename).
    pub fn write(&self, vault: &Path) -> Result<(), VaultError> {
        let path = Self::path_for(vault);
        let tmp = sidecar(&path, ".tmp");
        fs::write(&tmp, self.to_bytes())?;
        fs::File::open(&tmp)?.sync_all()?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Removes the device key file of the vault at `vault`, if there is one.
    pub fn remove(vault: &Path) -> Result<(), VaultError> {
        match fs::remove_file(Self::path_for(vault)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(VaultError::Io),
            _ => Ok(()),
        }
    }
}

/// A device key being set up: hand [`Self::key_bytes`] to the platform to
/// encrypt, then [`Self::finish`] with its ciphertext.
pub struct DeviceEnrollment {
    vault_id: VaultId,
    key: DeviceKey,
    wrapped_dek: Vec<u8>,
}

impl std::fmt::Debug for DeviceEnrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceEnrollment([REDACTED])")
    }
}

impl DeviceEnrollment {
    /// The raw device key, only for the platform's secure storage to encrypt.
    pub fn key_bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.key.expose().to_vec())
    }

    /// Builds the device key file around the platform's ciphertext of the key.
    pub fn finish(self, sealed_key: Vec<u8>) -> Result<DeviceKeyFile, VaultError> {
        if sealed_key.is_empty() || sealed_key.len() > MAX_SEALED_KEY_LEN {
            return Err(VaultError::DeviceKeyRejected);
        }
        Ok(DeviceKeyFile {
            vault_id: self.vault_id,
            wrapped_dek: self.wrapped_dek,
            sealed_key,
        })
    }
}

impl UnlockedVault {
    /// Starts setting up unlocking with a device key: a fresh random key that
    /// wraps this vault's DEK. Nothing is written yet.
    pub fn device_enrollment(&self) -> Result<DeviceEnrollment, VaultError> {
        let key = DeviceKey::generate()?;
        let wrapped_dek = wrap_dek_device(&key, self.vault_id(), self.dek())?;
        Ok(DeviceEnrollment {
            vault_id: *self.vault_id(),
            key,
            wrapped_dek,
        })
    }
}

/// Opens the vault with the device key the platform released after a biometric
/// check. `key` is zeroed. A key that does not open this vault (another vault,
/// a damaged file) is [`VaultError::DeviceKeyRejected`]; the caller should then
/// remove the file and ask for the password.
pub fn unlock_with_device_key(
    path: &Path,
    file: &DeviceKeyFile,
    key: &mut [u8],
) -> Result<UnlockedVault, VaultError> {
    let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
    let len_ok = key.len() == KEY_LEN;
    if len_ok {
        bytes.copy_from_slice(key);
    }
    zeroize::Zeroize::zeroize(key);
    if !len_ok {
        return Err(VaultError::DeviceKeyRejected);
    }
    let device_key = DeviceKey::from_bytes(&mut bytes);

    if !exists(path) {
        return Err(VaultError::NotFound);
    }
    let store = Store::open(path)?;
    let header = VaultHeader::load(&store)?;
    if header.vault_id != file.vault_id {
        return Err(VaultError::DeviceKeyRejected);
    }
    let dek =
        unwrap_dek_device(&device_key, &header.vault_id, &file.wrapped_dek).map_err(
            |e| match e {
                CryptoError::Decrypt | CryptoError::Malformed => VaultError::DeviceKeyRejected,
                other => other.into(),
            },
        )?;
    load(path, store, header, dek)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> DeviceKeyFile {
        DeviceKeyFile {
            vault_id: [0x11; 16],
            wrapped_dek: vec![0x22; WRAPPED_LEN],
            sealed_key: vec![0x33; 60],
        }
    }

    #[test]
    fn file_round_trip() {
        let f = file();
        let bytes = f.to_bytes();
        assert_eq!(&bytes[..5], b"MBDK\x01");
        assert_eq!(bytes.len(), HEADER_LEN + 60);
        assert_eq!(DeviceKeyFile::from_bytes(&bytes).unwrap(), f);
    }

    #[test]
    fn damaged_or_unknown_files_are_refused() {
        let bytes = file().to_bytes();
        assert_eq!(
            DeviceKeyFile::from_bytes(&bytes[..HEADER_LEN]).unwrap_err(),
            VaultError::Corrupted
        );
        assert_eq!(
            DeviceKeyFile::from_bytes(&bytes[..bytes.len() - 1]).unwrap_err(),
            VaultError::Corrupted
        );
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(
            DeviceKeyFile::from_bytes(&longer).unwrap_err(),
            VaultError::Corrupted
        );
        let mut magic = bytes.clone();
        magic[0] = b'X';
        assert_eq!(
            DeviceKeyFile::from_bytes(&magic).unwrap_err(),
            VaultError::Corrupted
        );
        let mut version = bytes.clone();
        version[4] = 2;
        assert_eq!(
            DeviceKeyFile::from_bytes(&version).unwrap_err(),
            VaultError::UnsupportedFormat
        );
        let mut too_long = file();
        too_long.sealed_key = vec![0; MAX_SEALED_KEY_LEN + 1];
        assert_eq!(
            DeviceKeyFile::from_bytes(&too_long.to_bytes()).unwrap_err(),
            VaultError::Corrupted
        );
        assert_eq!(
            DeviceKeyFile::from_bytes(&[]).unwrap_err(),
            VaultError::Corrupted
        );
    }

    #[test]
    fn debug_redacts() {
        assert_eq!(format!("{:?}", file()), "DeviceKeyFile([REDACTED])");
    }
}

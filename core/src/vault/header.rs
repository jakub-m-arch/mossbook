//! Vault header stored in the `meta` table (SPEC §7).

use crate::crypto::VaultId;
use crate::crypto::kdf::{KdfParams, SALT_LEN};
use crate::crypto::sealed::{PUBLIC_LEN, WRAPPED_SECRET_LEN};
use crate::crypto::wrap::WRAPPED_LEN;
use crate::store::Store;

use super::VaultError;

/// Vault format version written by this build.
pub const FORMAT_VERSION: u16 = 3;

const K_FORMAT_VERSION: &str = "format_version";
const K_VAULT_ID: &str = "vault_id";
const K_KDF_PARAMS: &str = "kdf_params";
const K_KDF_SALT: &str = "kdf_salt";
const K_WRAPPED_DEK_PWD: &str = "wrapped_dek_pwd";
const K_WRAPPED_DEK_REC: &str = "wrapped_dek_rec";
const K_INBOX_PUBLIC: &str = "x25519_pub";
const K_INBOX_SECRET: &str = "wrapped_x25519_priv";
const K_PASSWORD_EPOCH: &str = "password_epoch";

/// Inbox key pair for quick entries while locked (E-15). Optional: vaults
/// created before it existed get one on their next unlock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxKey {
    pub public: [u8; PUBLIC_LEN],
    /// Secret key wrapped under the DEK.
    pub wrapped_secret: Vec<u8>,
}

/// Public, non-secret vault metadata. Every field is either random or ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultHeader {
    pub vault_id: VaultId,
    pub kdf_params: KdfParams,
    pub kdf_salt: [u8; SALT_LEN],
    pub wrapped_dek_pwd: Vec<u8>,
    pub wrapped_dek_rec: Vec<u8>,
    pub inbox_key: Option<InboxKey>,
    /// How many times the password was changed (sync, D-22): of two devices'
    /// password wraps, the one with the higher count is the newer and is
    /// adopted. Not a time; absent (0) in vaults from before sync.
    pub password_epoch: u32,
}

impl VaultHeader {
    /// Reads and validates the header.
    pub fn load(store: &Store) -> Result<Self, VaultError> {
        let get = |key: &str| store.meta_get(key)?.ok_or(VaultError::Corrupted);
        let version = get(K_FORMAT_VERSION)?;
        let version = u16::from_le_bytes(
            version
                .as_slice()
                .try_into()
                .map_err(|_| VaultError::Corrupted)?,
        );
        if version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedFormat);
        }
        let header = Self {
            vault_id: fixed(&get(K_VAULT_ID)?)?,
            kdf_params: KdfParams::from_bytes(&get(K_KDF_PARAMS)?)
                .map_err(|_| VaultError::Corrupted)?,
            kdf_salt: fixed(&get(K_KDF_SALT)?)?,
            wrapped_dek_pwd: get(K_WRAPPED_DEK_PWD)?,
            wrapped_dek_rec: get(K_WRAPPED_DEK_REC)?,
            inbox_key: None,
            password_epoch: match store.meta_get(K_PASSWORD_EPOCH)? {
                Some(bytes) => u32::from_le_bytes(fixed(&bytes)?),
                None => 0,
            },
        };
        let mut header = header;
        header.inbox_key = match (
            store.meta_get(K_INBOX_PUBLIC)?,
            store.meta_get(K_INBOX_SECRET)?,
        ) {
            (Some(public), Some(wrapped_secret)) if wrapped_secret.len() == WRAPPED_SECRET_LEN => {
                Some(InboxKey {
                    public: fixed(&public)?,
                    wrapped_secret,
                })
            }
            (None, None) => None,
            _ => return Err(VaultError::Corrupted),
        };
        if header.wrapped_dek_pwd.len() != WRAPPED_LEN
            || header.wrapped_dek_rec.len() != WRAPPED_LEN
        {
            return Err(VaultError::Corrupted);
        }
        Ok(header)
    }

    /// Writes the whole header in one transaction.
    pub fn save(&self, store: &mut Store) -> Result<(), VaultError> {
        self.save_core(store)?;
        self.save_inbox_key(store)
    }

    /// Writes the inbox key pair, if any.
    pub fn save_inbox_key(&self, store: &mut Store) -> Result<(), VaultError> {
        if let Some(k) = &self.inbox_key {
            store.meta_set_all(&[
                (K_INBOX_PUBLIC, &k.public),
                (K_INBOX_SECRET, &k.wrapped_secret),
            ])?;
        }
        Ok(())
    }

    fn save_core(&self, store: &mut Store) -> Result<(), VaultError> {
        store.meta_set_all(&[
            (K_FORMAT_VERSION, &FORMAT_VERSION.to_le_bytes()),
            (K_VAULT_ID, &self.vault_id),
            (K_KDF_PARAMS, &self.kdf_params.to_bytes()),
            (K_KDF_SALT, &self.kdf_salt),
            (K_WRAPPED_DEK_PWD, &self.wrapped_dek_pwd),
            (K_WRAPPED_DEK_REC, &self.wrapped_dek_rec),
            (K_PASSWORD_EPOCH, &self.password_epoch.to_le_bytes()),
        ])?;
        Ok(())
    }

    /// Writes only the password wrap and its KDF inputs (password change, recovery reset).
    pub fn save_password_wrap(&self, store: &mut Store) -> Result<(), VaultError> {
        store.meta_set_all(&[
            (K_KDF_PARAMS, &self.kdf_params.to_bytes()),
            (K_KDF_SALT, &self.kdf_salt),
            (K_WRAPPED_DEK_PWD, &self.wrapped_dek_pwd),
            (K_PASSWORD_EPOCH, &self.password_epoch.to_le_bytes()),
        ])?;
        Ok(())
    }
}

fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], VaultError> {
    bytes.try_into().map_err(|_| VaultError::Corrupted)
}

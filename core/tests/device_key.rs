//! Integration tests for unlocking with a device key (fingerprint unlock, V-7).
//! The platform's encryption of the key (Android Keystore) is stood in for by a
//! reversible transform: the core treats that ciphertext as opaque bytes.
//! All data is synthetic.

use std::path::PathBuf;

use mossbook_core::crypto::aead::RecordType;
use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::vault::{self, DeviceKeyFile, PendingVault, UnlockedVault, VaultError};
use tempfile::TempDir;

const PASSWORD: &str = "correct horse battery";

struct Fixture {
    dir: TempDir,
    path: PathBuf,
    phrase: String,
}

fn create_vault_in(dir: TempDir) -> (Fixture, UnlockedVault) {
    let path = dir.path().join("vault.db");
    let mut pending =
        PendingVault::with_kdf_params(PASSWORD, KdfParams::new(64, 1, 1).unwrap()).unwrap();
    let phrase = pending.recovery_phrase().to_string();
    let words: Vec<&str> = phrase.split(' ').collect();
    let answers: Vec<(usize, &str)> = pending
        .confirmation_positions()
        .iter()
        .map(|&i| (i, words[i]))
        .collect();
    assert!(pending.confirm(&answers));
    let vault = pending.write(&path).unwrap();
    (Fixture { dir, path, phrase }, vault)
}

fn create_vault() -> (Fixture, UnlockedVault) {
    create_vault_in(tempfile::tempdir().unwrap())
}

/// Stand-in for the platform: "seals" by reversing and flipping the bytes.
fn platform_seal(key: &[u8]) -> Vec<u8> {
    key.iter().rev().map(|b| b ^ 0xa5).collect()
}

fn platform_open(sealed: &[u8]) -> Vec<u8> {
    sealed.iter().rev().map(|b| b ^ 0xa5).collect()
}

/// Sets up the device key the way the app does: enroll, seal, write.
fn enroll(fx: &Fixture, vault: &UnlockedVault) {
    let enrollment = vault.device_enrollment().unwrap();
    let sealed = platform_seal(&enrollment.key_bytes());
    enrollment.finish(sealed).unwrap().write(&fx.path).unwrap();
}

fn unlock_with_device(fx: &Fixture) -> Result<UnlockedVault, VaultError> {
    let file = DeviceKeyFile::read(&fx.path)
        .unwrap()
        .expect("device key file");
    let mut key = platform_open(file.sealed_key());
    let result = vault::unlock_with_device_key(&fx.path, &file, &mut key);
    assert!(key.iter().all(|&b| b == 0), "the key buffer is zeroed");
    result
}

#[test]
fn device_key_opens_the_vault_with_its_records() {
    let (fx, mut v) = create_vault();
    v.put([1; 16], RecordType::Entry, b"synthetic entry")
        .unwrap();
    enroll(&fx, &v);
    let vault_id = *v.vault_id();
    v.lock().unwrap();

    let mut v = unlock_with_device(&fx).unwrap();
    assert_eq!(v.vault_id(), &vault_id);
    let records = v.take_loaded();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].plaintext.as_slice(), b"synthetic entry");
}

#[test]
fn no_file_means_no_device_unlock() {
    let (fx, v) = create_vault();
    assert_eq!(DeviceKeyFile::read(&fx.path).unwrap(), None);
    // Removing a file that is not there is fine.
    DeviceKeyFile::remove(&fx.path).unwrap();
    v.lock().unwrap();
}

#[test]
fn a_wrong_or_short_key_is_rejected() {
    let (fx, v) = create_vault();
    enroll(&fx, &v);
    v.lock().unwrap();
    let file = DeviceKeyFile::read(&fx.path).unwrap().unwrap();

    let mut wrong = platform_open(file.sealed_key());
    wrong[0] ^= 0x01;
    assert_eq!(
        vault::unlock_with_device_key(&fx.path, &file, &mut wrong).unwrap_err(),
        VaultError::DeviceKeyRejected
    );
    let mut short = platform_open(file.sealed_key());
    short.pop();
    assert_eq!(
        vault::unlock_with_device_key(&fx.path, &file, &mut short).unwrap_err(),
        VaultError::DeviceKeyRejected
    );
    assert!(
        short.iter().all(|&b| b == 0),
        "rejected keys are zeroed too"
    );
    // The password still works.
    vault::unlock(&fx.path, PASSWORD).unwrap();
}

#[test]
fn a_device_key_of_another_vault_is_rejected() {
    let (a, va) = create_vault();
    enroll(&a, &va);
    va.lock().unwrap();
    let (b, vb) = create_vault();
    vb.lock().unwrap();

    let file = DeviceKeyFile::read(&a.path).unwrap().unwrap();
    let mut key = platform_open(file.sealed_key());
    assert_eq!(
        vault::unlock_with_device_key(&b.path, &file, &mut key).unwrap_err(),
        VaultError::DeviceKeyRejected
    );
}

#[test]
fn a_tampered_file_does_not_open() {
    let (fx, v) = create_vault();
    enroll(&fx, &v);
    v.lock().unwrap();
    let path = DeviceKeyFile::path_for(&fx.path);
    let original = std::fs::read(&path).unwrap();
    // Every byte of the wrapped DEK and the vault id matters.
    for i in 5..5 + 16 + 72 {
        let mut bytes = original.clone();
        bytes[i] ^= 0x01;
        std::fs::write(&path, &bytes).unwrap();
        let file = DeviceKeyFile::read(&fx.path).unwrap().unwrap();
        let mut key = platform_open(file.sealed_key());
        assert_eq!(
            vault::unlock_with_device_key(&fx.path, &file, &mut key).unwrap_err(),
            VaultError::DeviceKeyRejected,
            "byte {i}"
        );
    }
    std::fs::write(&path, &original[..original.len() - 1]).unwrap();
    assert_eq!(
        DeviceKeyFile::read(&fx.path).unwrap_err(),
        VaultError::Corrupted
    );
}

#[test]
fn password_change_and_recovery_keep_the_device_key() {
    let (fx, mut v) = create_vault();
    enroll(&fx, &v);
    v.change_password(PASSWORD, "another synthetic password")
        .unwrap();
    v.lock().unwrap();
    unlock_with_device(&fx).unwrap().lock().unwrap();

    vault::unlock_with_recovery(&fx.path, &fx.phrase, "third synthetic password")
        .unwrap()
        .lock()
        .unwrap();
    unlock_with_device(&fx).unwrap();
}

#[test]
fn enrolling_again_replaces_the_file() {
    let (fx, v) = create_vault();
    enroll(&fx, &v);
    let first = std::fs::read(DeviceKeyFile::path_for(&fx.path)).unwrap();
    enroll(&fx, &v);
    let second = std::fs::read(DeviceKeyFile::path_for(&fx.path)).unwrap();
    assert_ne!(first, second, "a fresh device key each time");
    v.lock().unwrap();
    unlock_with_device(&fx).unwrap();
    // No temporary file is left behind.
    let names: Vec<_> = std::fs::read_dir(fx.dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.contains("device-key"))
        .collect();
    assert_eq!(names, ["vault.db.device-key"]);
}

#[test]
fn removing_the_file_turns_device_unlock_off() {
    let (fx, v) = create_vault();
    enroll(&fx, &v);
    DeviceKeyFile::remove(&fx.path).unwrap();
    assert_eq!(DeviceKeyFile::read(&fx.path).unwrap(), None);
    v.lock().unwrap();
}

#[test]
fn destroy_removes_the_device_key_file() {
    let (fx, v) = create_vault();
    enroll(&fx, &v);
    v.destroy(PASSWORD).unwrap();
    assert!(!DeviceKeyFile::path_for(&fx.path).exists());
    let leftovers: Vec<_> = std::fs::read_dir(fx.dir.path()).unwrap().collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn rejected_sealed_keys() {
    let (_fx, v) = create_vault();
    assert_eq!(
        v.device_enrollment()
            .unwrap()
            .finish(Vec::new())
            .unwrap_err(),
        VaultError::DeviceKeyRejected
    );
    assert_eq!(
        v.device_enrollment()
            .unwrap()
            .finish(vec![0; vault::MAX_SEALED_KEY_LEN + 1])
            .unwrap_err(),
        VaultError::DeviceKeyRejected
    );
}

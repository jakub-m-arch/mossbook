//! Integration tests for the vault lifecycle on a real SQLite file.
//! All data is synthetic.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mossbook_core::crypto::aead::RecordType;
use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::vault::{self, DestroyError, PendingVault, UnlockedVault, VaultError};
use rusqlite::Connection;
use tempfile::TempDir;

const PASSWORD: &str = "correct horse battery";
const MARKER: &[u8] = b"SYNTHETIC-PLAINTEXT-MARKER";

fn fast() -> KdfParams {
    KdfParams::new(64, 1, 1).unwrap()
}

struct Fixture {
    _dir: TempDir,
    path: PathBuf,
    phrase: String,
}

/// Creates a confirmed vault on disk and returns it unlocked.
fn create_vault() -> (Fixture, UnlockedVault) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let mut pending = PendingVault::with_kdf_params(PASSWORD, fast()).unwrap();
    let phrase = pending.recovery_phrase().to_string();
    confirm(&mut pending, &phrase);
    let vault = pending.write(&path).unwrap();
    (
        Fixture {
            _dir: dir,
            path,
            phrase,
        },
        vault,
    )
}

fn confirm(pending: &mut PendingVault, phrase: &str) {
    let words: Vec<&str> = phrase.split(' ').collect();
    let answers: Vec<(usize, &str)> = pending
        .confirmation_positions()
        .iter()
        .map(|&i| (i, words[i]))
        .collect();
    assert!(pending.confirm(&answers));
}

/// Decrypted records of a freshly unlocked vault, by id.
fn loaded(v: &mut UnlockedVault) -> HashMap<[u8; 16], (RecordType, Vec<u8>)> {
    v.take_loaded()
        .into_iter()
        .map(|r| (r.id, (r.record_type, r.plaintext.to_vec())))
        .collect()
}

fn raw_file_bytes(path: &Path) -> Vec<u8> {
    let mut bytes = std::fs::read(path).unwrap();
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    if let Ok(w) = std::fs::read(wal) {
        bytes.extend(w);
    }
    bytes
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn nothing_is_written_before_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let mut pending = PendingVault::with_kdf_params(PASSWORD, fast()).unwrap();
    let phrase = pending.recovery_phrase().to_string();
    let words: Vec<&str> = phrase.split(' ').collect();
    let pos = pending.confirmation_positions();

    // Wrong word, too few words, or other positions: not confirmed.
    assert!(!pending.confirm(&[
        (pos[0], words[pos[0]]),
        (pos[1], words[pos[1]]),
        (pos[2], "zoo")
    ]));
    assert!(!pending.confirm(&[(pos[0], words[pos[0]]), (pos[1], words[pos[1]])]));
    let others: Vec<(usize, &str)> = (0..24)
        .filter(|i| !pos.contains(i))
        .take(3)
        .map(|i| (i, words[i]))
        .collect();
    assert!(!pending.confirm(&others));
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());

    let err = pending.write(&path).unwrap_err();
    assert_eq!(err, VaultError::NotConfirmed);
    assert!(!path.exists());
}

#[test]
fn short_password_is_rejected() {
    assert_eq!(
        PendingVault::with_kdf_params("1234567", fast()).unwrap_err(),
        VaultError::PasswordTooShort
    );
    // 8 characters, not bytes: multi-byte characters count once.
    assert!(PendingVault::with_kdf_params("zażółćgę", fast()).is_ok());
}

#[test]
fn records_survive_lock_and_unlock() {
    let (fx, mut v) = create_vault();
    v.put([1; 16], RecordType::Entry, b"first entry").unwrap();
    v.put([2; 16], RecordType::Tag, b"tag").unwrap();
    v.put([1; 16], RecordType::Entry, b"first entry, edited")
        .unwrap();
    v.put([3; 16], RecordType::Entry, b"to delete").unwrap();
    assert!(v.delete(&[3; 16]).unwrap());
    let vault_id = *v.vault_id();
    v.lock().unwrap();

    let mut v = vault::unlock(&fx.path, PASSWORD).unwrap();
    assert_eq!(v.vault_id(), &vault_id);
    let records = loaded(&mut v);
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[&[1; 16]],
        (RecordType::Entry, b"first entry, edited".to_vec())
    );
    assert_eq!(records[&[2; 16]], (RecordType::Tag, b"tag".to_vec()));
    assert_eq!(v.unreadable_records(), 0);
    // Plaintext is handed out once; the vault keeps no copy.
    assert!(v.take_loaded().is_empty());
}

#[test]
fn wrong_password_and_missing_vault() {
    let (fx, v) = create_vault();
    v.lock().unwrap();
    assert_eq!(
        vault::unlock(&fx.path, "wrong password").unwrap_err(),
        VaultError::WrongPassword
    );
    assert_eq!(
        vault::unlock(&fx.path, "").unwrap_err(),
        VaultError::WrongPassword
    );
    let missing = fx.path.with_file_name("missing.db");
    assert_eq!(
        vault::unlock(&missing, PASSWORD).unwrap_err(),
        VaultError::NotFound
    );
    assert!(!missing.exists());
}

#[test]
fn creating_over_existing_vault_is_refused() {
    let (fx, v) = create_vault();
    v.lock().unwrap();
    let mut pending = PendingVault::with_kdf_params(PASSWORD, fast()).unwrap();
    let phrase = pending.recovery_phrase().to_string();
    confirm(&mut pending, &phrase);
    assert_eq!(
        pending.write(&fx.path).unwrap_err(),
        VaultError::AlreadyExists
    );
    assert!(vault::unlock(&fx.path, PASSWORD).is_ok());
}

#[test]
fn stale_temp_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    std::fs::write(
        dir.path().join("vault.db.creating"),
        b"garbage from a crash",
    )
    .unwrap();
    let mut pending = PendingVault::with_kdf_params(PASSWORD, fast()).unwrap();
    let phrase = pending.recovery_phrase().to_string();
    confirm(&mut pending, &phrase);
    pending.write(&path).unwrap().lock().unwrap();
    assert!(!dir.path().join("vault.db.creating").exists());
    assert!(vault::unlock(&path, PASSWORD).is_ok());
}

#[test]
fn change_password_keeps_data_and_recovery() {
    let (fx, mut v) = create_vault();
    v.put([1; 16], RecordType::Entry, b"kept").unwrap();
    assert_eq!(
        v.change_password("not the password", "new password 1")
            .unwrap_err(),
        VaultError::WrongPassword
    );
    assert_eq!(
        v.change_password(PASSWORD, "short").unwrap_err(),
        VaultError::PasswordTooShort
    );
    v.change_password(PASSWORD, "new password 1").unwrap();
    v.lock().unwrap();

    assert_eq!(
        vault::unlock(&fx.path, PASSWORD).unwrap_err(),
        VaultError::WrongPassword
    );
    let mut v = vault::unlock(&fx.path, "new password 1").unwrap();
    assert_eq!(loaded(&mut v)[&[1; 16]].1, b"kept");
    v.lock().unwrap();

    // The recovery key still works after a password change.
    let mut v = vault::unlock_with_recovery(&fx.path, &fx.phrase, "after recovery").unwrap();
    assert_eq!(loaded(&mut v)[&[1; 16]].1, b"kept");
}

#[test]
fn recovery_sets_new_password() {
    let (fx, mut v) = create_vault();
    v.put([1; 16], RecordType::Entry, b"kept").unwrap();
    v.lock().unwrap();

    let messy = format!("  {}  ", fx.phrase.to_uppercase().replace(' ', "\n"));
    let mut v = vault::unlock_with_recovery(&fx.path, &messy, "brand new password").unwrap();
    assert_eq!(loaded(&mut v)[&[1; 16]].1, b"kept");
    v.lock().unwrap();

    assert_eq!(
        vault::unlock(&fx.path, PASSWORD).unwrap_err(),
        VaultError::WrongPassword
    );
    assert!(vault::unlock(&fx.path, "brand new password").is_ok());
}

#[test]
fn recovery_errors() {
    let (fx, v) = create_vault();
    v.lock().unwrap();
    let (other, other_vault) = create_vault();
    other_vault.lock().unwrap();

    assert_eq!(
        vault::unlock_with_recovery(&fx.path, &other.phrase, "new password").unwrap_err(),
        VaultError::WrongRecoveryKey
    );
    assert_eq!(
        vault::unlock_with_recovery(&fx.path, "not a phrase", "new password").unwrap_err(),
        VaultError::InvalidRecoveryPhrase
    );
    assert_eq!(
        vault::unlock_with_recovery(&fx.path, &fx.phrase, "short").unwrap_err(),
        VaultError::PasswordTooShort
    );
    // A failed recovery leaves the old password in place.
    assert!(vault::unlock(&fx.path, PASSWORD).is_ok());
}

#[test]
fn verify_recovery_checks_without_changing_anything() {
    let (fx, v) = create_vault();
    let (other, _other_vault) = create_vault();
    let messy = format!(" {} ", fx.phrase.to_uppercase());
    assert_eq!(v.verify_recovery(&messy), Ok(()));
    assert_eq!(
        v.verify_recovery(&other.phrase),
        Err(VaultError::WrongRecoveryKey)
    );
    assert_eq!(
        v.verify_recovery("not a phrase"),
        Err(VaultError::InvalidRecoveryPhrase)
    );
    // One word changed: still 24 words, but the checksum (or the key) fails.
    let mut words: Vec<&str> = fx.phrase.split(' ').collect();
    words[0] = if words[0] == "abandon" {
        "ability"
    } else {
        "abandon"
    };
    assert!(v.verify_recovery(&words.join(" ")).is_err());
    v.lock().unwrap();
    assert!(vault::unlock(&fx.path, PASSWORD).is_ok());
}

#[test]
fn destroy_requires_password_and_removes_files() {
    let (fx, v) = create_vault();
    let v = match v.destroy("wrong password") {
        Err(DestroyError::Rejected { vault, error }) => {
            assert_eq!(error, VaultError::WrongPassword);
            *vault
        }
        other => panic!("expected rejection, got {other:?}"),
    };
    assert!(fx.path.exists());
    v.destroy(PASSWORD).unwrap();
    let leftovers: Vec<_> = std::fs::read_dir(fx.path.parent().unwrap())
        .unwrap()
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    assert!(!vault::exists(&fx.path));
}

#[test]
fn no_plaintext_on_disk() {
    let (fx, mut v) = create_vault();
    v.put([1; 16], RecordType::Entry, MARKER).unwrap();
    // Checked while open (WAL in use) and after lock (checkpointed).
    assert!(!contains(&raw_file_bytes(&fx.path), MARKER));
    assert!(!contains(&raw_file_bytes(&fx.path), PASSWORD.as_bytes()));
    v.lock().unwrap();
    let bytes = raw_file_bytes(&fx.path);
    assert!(!contains(&bytes, MARKER));
    assert!(!contains(&bytes, PASSWORD.as_bytes()));
    // Single BIP-39 words also occur in the plain SQLite schema ("table",
    // "value"…), so look for the phrase and every run of three words instead.
    let words: Vec<&str> = fx.phrase.split(' ').collect();
    assert!(!contains(&bytes, fx.phrase.as_bytes()));
    for run in words.windows(3) {
        assert!(
            !contains(&bytes, run.join(" ").as_bytes()),
            "recovery phrase on disk"
        );
    }
}

#[test]
fn tampered_records_are_isolated() {
    let (fx, mut v) = create_vault();
    for i in 1..=4u8 {
        v.put([i; 16], RecordType::Entry, &[i; 8]).unwrap();
    }
    v.lock().unwrap();
    {
        let db = Connection::open(&fx.path).unwrap();
        // Flip a byte in record 1.
        db.execute(
            "UPDATE records SET blob = CAST(x'00' || substr(blob, 2) AS BLOB) WHERE id = ?1",
            [&[1u8; 16][..]],
        )
        .unwrap();
        // Swap the blobs of records 2 and 3.
        let b2: Vec<u8> = db
            .query_row(
                "SELECT blob FROM records WHERE id = ?1",
                [&[2u8; 16][..]],
                |r| r.get(0),
            )
            .unwrap();
        let b3: Vec<u8> = db
            .query_row(
                "SELECT blob FROM records WHERE id = ?1",
                [&[3u8; 16][..]],
                |r| r.get(0),
            )
            .unwrap();
        db.execute(
            "UPDATE records SET blob = ?1 WHERE id = ?2",
            (&b3, &[2u8; 16][..]),
        )
        .unwrap();
        db.execute(
            "UPDATE records SET blob = ?1 WHERE id = ?2",
            (&b2, &[3u8; 16][..]),
        )
        .unwrap();
        // Change the type of record 4 and add a row with an unknown type.
        db.execute(
            "UPDATE records SET type = 2 WHERE id = ?1",
            [&[4u8; 16][..]],
        )
        .unwrap();
        db.execute(
            "INSERT INTO records (id, type, blob) VALUES (?1, 9, x'00')",
            [&[5u8; 16][..]],
        )
        .unwrap();
    }
    let mut v = vault::unlock(&fx.path, PASSWORD).unwrap();
    assert_eq!(v.unreadable_records(), 5);
    assert!(loaded(&mut v).is_empty());
}

#[test]
fn tampered_header_is_detected() {
    let cases: [(&str, &str, VaultError); 5] = [
        (
            "UPDATE meta SET value = randomblob(16) WHERE key = 'vault_id'",
            "vault_id",
            VaultError::WrongPassword,
        ),
        (
            "DELETE FROM meta WHERE key = 'kdf_salt'",
            "salt",
            VaultError::Corrupted,
        ),
        (
            "UPDATE meta SET value = x'0400' WHERE key = 'format_version'",
            "newer version",
            VaultError::UnsupportedFormat,
        ),
        (
            // Schema 3 always comes with format 3; a mismatch is not guessed at.
            "UPDATE meta SET value = x'0100' WHERE key = 'format_version'",
            "older version label",
            VaultError::UnsupportedFormat,
        ),
        (
            "UPDATE meta SET value = x'01ffffffff0100000001000000' WHERE key = 'kdf_params'",
            "params",
            VaultError::Corrupted,
        ),
    ];
    for (sql, name, expected) in cases {
        let (fx, v) = create_vault();
        v.lock().unwrap();
        Connection::open(&fx.path)
            .unwrap()
            .execute(sql, [])
            .unwrap();
        assert_eq!(
            vault::unlock(&fx.path, PASSWORD).unwrap_err(),
            expected,
            "{name}"
        );
    }
}

/// A vault file written by format version 1 (Mossbook 0.1–0.3) must keep
/// opening in every later build. Frozen: this build writes version 2.
const FIXTURE_V1: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault-v1.db");
const FIXTURE_PASSWORD: &str = "fixture password v1";

#[test]
fn opens_format_v1_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    std::fs::copy(FIXTURE_V1, &path).unwrap();
    let mut v = vault::unlock(&path, FIXTURE_PASSWORD).unwrap();
    assert_eq!(v.unreadable_records(), 0);
    let records = loaded(&mut v);
    assert_eq!(
        records[&[1; 16]],
        (RecordType::Entry, b"fixture entry".to_vec())
    );
    assert_eq!(
        records[&[2; 16]],
        (RecordType::Settings, b"fixture settings".to_vec())
    );
    // Migrated to format 2 on open: the recordings table exists and works.
    v.put_media([3; 16], RecordType::Voice, b"synthetic recording")
        .unwrap();
    v.lock().unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let schema: u32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    let format: Vec<u8> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'format_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(schema, 3);
    assert_eq!(format, 3u16.to_le_bytes());
    drop(conn);
    let v = vault::unlock(&path, FIXTURE_PASSWORD).unwrap();
    assert_eq!(
        v.open_media(&[3; 16]).unwrap().unwrap().as_slice(),
        b"synthetic recording"
    );
}

#[test]
fn a_vault_from_a_newer_build_is_refused_untouched() {
    let (fx, v) = create_vault();
    v.lock().unwrap();
    let conn = rusqlite::Connection::open(&fx.path).unwrap();
    conn.pragma_update(None, "user_version", 4).unwrap();
    drop(conn);
    let before = std::fs::read(&fx.path).unwrap();
    assert!(vault::unlock(&fx.path, PASSWORD).is_err());
    assert_eq!(std::fs::read(&fx.path).unwrap(), before);
}

#[test]
fn recordings_are_bound_to_their_place() {
    let (fx, mut v) = create_vault();
    v.put_media([1; 16], RecordType::Voice, b"synthetic recording A")
        .unwrap();
    // Only media types go into the media table.
    assert!(v.put_media([2; 16], RecordType::Entry, b"x").is_err());
    v.lock().unwrap();
    let conn = rusqlite::Connection::open(&fx.path).unwrap();
    // The same blob under another id, and as a record.
    conn.execute(
        "INSERT INTO media (id, type, blob) SELECT x'09090909090909090909090909090909', type, blob FROM media",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO records (id, type, blob) SELECT id, type, blob FROM media WHERE id = x'01010101010101010101010101010101'",
        [],
    )
    .unwrap();
    drop(conn);

    let mut v = vault::unlock(&fx.path, PASSWORD).unwrap();
    assert_eq!(v.unreadable_records(), 1, "a recording among the records");
    assert!(v.take_loaded().is_empty());
    assert_eq!(
        v.open_media(&[1; 16]).unwrap().unwrap().as_slice(),
        b"synthetic recording A"
    );
    assert_eq!(v.open_media(&[9; 16]), Err(VaultError::Corrupted));
    assert_eq!(v.open_media(&[7; 16]), Ok(None));
}

/// A vault file written by format version 2 (voice notes, E-18, Mossbook 0.4).
/// Frozen: this build writes version 3.
const FIXTURE_V2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault-v2.db");
const FIXTURE_V2_PASSWORD: &str = "fixture password v2";

#[test]
fn opens_format_v2_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    std::fs::copy(FIXTURE_V2, &path).unwrap();
    let mut v = vault::unlock(&path, FIXTURE_V2_PASSWORD).unwrap();
    assert_eq!(v.unreadable_records(), 0);
    assert_eq!(
        loaded(&mut v)[&[1; 16]],
        (RecordType::Entry, b"fixture entry".to_vec())
    );
    assert_eq!(
        v.open_media(&[3; 16]).unwrap().unwrap().as_slice(),
        b"fixture recording"
    );
}

/// A vault file written by format version 3 (sync, D-22): an entry, a
/// recording, a tombstone and a password changed once.
/// Regenerate only on purpose: `cargo test -p mossbook-core -- --ignored generate_v3_fixture`.
const FIXTURE_V3: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault-v3.db");
const FIXTURE_V3_PASSWORD: &str = "fixture password v3";

#[test]
fn opens_format_v3_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    std::fs::copy(FIXTURE_V3, &path).unwrap();
    let mut v = vault::unlock(&path, FIXTURE_V3_PASSWORD).unwrap();
    assert_eq!(v.unreadable_records(), 0);
    let records = loaded(&mut v);
    assert_eq!(
        records[&[1; 16]],
        (RecordType::Entry, b"fixture entry".to_vec())
    );
    assert_eq!(
        records[&[2; 16]],
        (RecordType::Tombstone, b"fixture tombstone".to_vec())
    );
    assert_eq!(
        v.open_media(&[3; 16]).unwrap().unwrap().as_slice(),
        b"fixture recording"
    );
}

#[test]
#[ignore = "writes tests/fixtures/vault-v3.db"]
fn generate_v3_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let mut pending = PendingVault::with_kdf_params("first password", fast()).unwrap();
    let phrase = pending.recovery_phrase().to_string();
    confirm(&mut pending, &phrase);
    let mut v = pending.write(&path).unwrap();
    v.change_password("first password", FIXTURE_V3_PASSWORD)
        .unwrap();
    v.put([1; 16], RecordType::Entry, b"fixture entry").unwrap();
    v.put([2; 16], RecordType::Tombstone, b"fixture tombstone")
        .unwrap();
    v.put_media([3; 16], RecordType::Voice, b"fixture recording")
        .unwrap();
    v.lock().unwrap();
    std::fs::copy(&path, FIXTURE_V3).unwrap();
}

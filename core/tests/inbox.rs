//! Quick entries while locked (E-15): append-only inbox sealed to the vault's
//! public key. Synthetic data only.

use std::path::{Path, PathBuf};

use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::domain::{EntryKindTag, InboxEntry, Timestamp, encode};
use mossbook_core::journal::{Journal, JournalError};
use mossbook_core::query::EntryFilter;
use mossbook_core::vault::{self, PendingVault, VaultError};
use rusqlite::Connection;
use tempfile::TempDir;
use time::OffsetDateTime;
use time::macros::datetime;

const PASSWORD: &str = "correct horse battery";
const NOW: OffsetDateTime = datetime!(2026-10-20 08:00 +02:00);

fn locked_vault() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
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
    pending.write(&path).unwrap().lock().unwrap();
    (dir, path)
}

fn inbox_rows(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM inbox", [], |r| r.get(0))
        .unwrap()
}

fn open(path: &Path) -> Journal {
    Journal::open(vault::unlock(path, PASSWORD).unwrap())
}

#[test]
fn locked_quick_entries_arrive_after_unlock() {
    let (_dir, path) = locked_vault();
    Journal::quick_entry_locked(&path, EntryKindTag::Fear, Some(7), NOW).unwrap();
    Journal::quick_entry_locked(&path, EntryKindTag::Mood, Some(4), NOW).unwrap();
    Journal::quick_entry_locked(&path, EntryKindTag::Nightmare, None, NOW).unwrap();
    assert_eq!(inbox_rows(&path), 3);

    let j = open(&path);
    let entries = j.entries(&EntryFilter::default());
    assert_eq!(entries.len(), 3);
    assert!(
        entries
            .iter()
            .all(|e| e.incomplete && e.occurred_at == Timestamp(NOW))
    );
    assert!(
        entries
            .iter()
            .any(|e| e.kind.tag() == EntryKindTag::Fear && e.intensity == Some(7))
    );
    assert!(
        entries
            .iter()
            .any(|e| e.kind.tag() == EntryKindTag::Mood && e.mood == Some(4))
    );
    assert!(
        entries
            .iter()
            .any(|e| e.kind.tag() == EntryKindTag::Nightmare && e.sensitive)
    );
    assert_eq!(j.vault().unreadable_records(), 0);
    j.lock().unwrap();

    // Moved for good: the inbox is empty and the entries are regular records now.
    assert_eq!(inbox_rows(&path), 0);
    assert_eq!(open(&path).entries(&EntryFilter::default()).len(), 3);
}

#[test]
fn invalid_values_are_rejected_before_writing() {
    let (_dir, path) = locked_vault();
    let err = Journal::quick_entry_locked(&path, EntryKindTag::Mood, Some(9), NOW).unwrap_err();
    assert!(matches!(err, JournalError::Domain(_)));
    // A session is written in the form only (D-26).
    let err = Journal::quick_entry_locked(&path, EntryKindTag::Session, None, NOW).unwrap_err();
    assert!(matches!(err, JournalError::Domain(_)));
    assert_eq!(inbox_rows(&path), 0);
}

#[test]
fn the_inbox_holds_no_plaintext() {
    let (_dir, path) = locked_vault();
    Journal::quick_entry_locked(&path, EntryKindTag::Nightmare, None, NOW).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    for needle in [&b"nightmare"[..], b"2026-10-20"] {
        assert!(!bytes.windows(needle.len()).any(|w| w == needle));
    }
}

#[test]
fn older_vaults_get_an_inbox_key_on_unlock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vault-v1.db"),
        &path,
    )
    .unwrap();
    let err = Journal::quick_entry_locked(&path, EntryKindTag::Note, None, NOW).unwrap_err();
    assert_eq!(err, JournalError::Vault(VaultError::InboxUnavailable));

    vault::unlock(&path, "fixture password v1")
        .unwrap()
        .lock()
        .unwrap();
    Journal::quick_entry_locked(&path, EntryKindTag::Note, None, NOW).unwrap();
    let j = Journal::open(vault::unlock(&path, "fixture password v1").unwrap());
    assert!(
        j.entries(&EntryFilter::default())
            .iter()
            .any(|e| e.kind.tag() == EntryKindTag::Note)
    );
}

#[test]
fn garbage_and_out_of_range_items_are_not_trusted() {
    let (_dir, path) = locked_vault();
    Journal::quick_entry_locked(&path, EntryKindTag::Fear, Some(5), NOW).unwrap();
    // Garbage that is not a sealed box for this vault.
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO inbox (id, blob) VALUES (randomblob(16), randomblob(80))",
            [],
        )
        .unwrap();
    // A correctly sealed item with a value out of range (bypassing the API).
    let hostile = InboxEntry {
        kind: EntryKindTag::Mood,
        value: Some(200),
        occurred_at: Timestamp(NOW),
    };
    vault::append_to_inbox(&path, &encode(&hostile).unwrap()).unwrap();
    // A sealed session, which quick entry never writes.
    let session = InboxEntry {
        kind: EntryKindTag::Session,
        value: None,
        occurred_at: Timestamp(NOW),
    };
    vault::append_to_inbox(&path, &encode(&session).unwrap()).unwrap();

    let j = open(&path);
    assert_eq!(
        j.entries(&EntryFilter::default()).len(),
        1,
        "only the valid entry"
    );
    assert_eq!(j.vault().unreadable_records(), 3);
}

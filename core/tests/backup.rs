//! Backup export, restore and import (X-1, X-2, SPEC §10). Synthetic data only.

mod common;

use std::path::{Path, PathBuf};

use mossbook_core::backup::{Backup, BackupError, MAGIC};
use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::domain::{
    AutoLock, Contact, Entry, EntryKind, EntryKindTag, SafetyPlan, Settings, Timestamp,
};
use mossbook_core::journal::{ImportMode, ImportReport, Journal};
use mossbook_core::query::EntryFilter;
use mossbook_core::vault::{self, PendingVault};
use tempfile::TempDir;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

const PASSWORD: &str = "correct horse battery";
const NOW: OffsetDateTime = datetime!(2026-10-19 20:00 +02:00);
const MARKER: &str = "SYNTHETIC-BACKUP-MARKER";

struct Fixture {
    _dir: TempDir,
    path: PathBuf,
    phrase: String,
}

fn create(password: &str) -> (Fixture, Journal) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let mut pending =
        PendingVault::with_kdf_params(password, KdfParams::new(64, 1, 1).unwrap()).unwrap();
    let phrase = pending.recovery_phrase().to_string();
    let words: Vec<&str> = phrase.split(' ').collect();
    let answers: Vec<(usize, &str)> = pending
        .confirmation_positions()
        .iter()
        .map(|&i| (i, words[i]))
        .collect();
    assert!(pending.confirm(&answers));
    let journal = Journal::open(pending.write(&path).unwrap());
    (
        Fixture {
            _dir: dir,
            path,
            phrase,
        },
        journal,
    )
}

/// A journal with two entries, a tag, a plan and settings.
fn populated() -> (Fixture, Journal) {
    let (fx, mut j) = create(PASSWORD);
    let tag = j.create_tag("Praca", "#4f6b47").unwrap().id;
    let mut e = Entry::new(EntryKind::empty(EntryKindTag::Fear), NOW).unwrap();
    e.note = MARKER.into();
    e.intensity = Some(7);
    e.tags = vec![tag];
    j.save_entry(e, NOW).unwrap();
    j.quick_entry(EntryKindTag::Mood, Some(4), NOW).unwrap();
    let mut plan = SafetyPlan::default();
    plan.self_coping = vec!["walk".into()];
    let mut c = Contact::default();
    c.name = "Synthetic Friend".into();
    c.phone = "+48 000 000 000".into();
    plan.contacts = vec![c];
    j.save_plan(plan).unwrap();
    let mut settings = Settings::default();
    settings.autolock = AutoLock::After5Min;
    j.save_settings(settings).unwrap();
    (fx, j)
}

fn export(j: &Journal) -> Vec<u8> {
    j.vault().export_backup().unwrap().to_bytes().unwrap()
}

/// Comparable view of everything in a journal.
fn snapshot(j: &Journal) -> (Vec<Entry>, Vec<(String, String)>, SafetyPlan, Settings) {
    let mut entries: Vec<Entry> = j
        .entries(&EntryFilter::default())
        .into_iter()
        .cloned()
        .collect();
    entries.sort_by_key(|e| e.id);
    let tags = j
        .tags()
        .iter()
        .map(|t| (t.name.clone(), t.color.clone()))
        .collect();
    (entries, tags, j.plan().clone(), j.settings().clone())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn export_then_restore_gives_identical_data() {
    let (fx, j) = populated();
    let bytes = export(&j);
    assert!(bytes.starts_with(MAGIC));
    assert!(
        !contains(&bytes, MARKER.as_bytes()),
        "no plaintext in the backup"
    );
    assert!(!contains(&bytes, b"Synthetic Friend"));
    let before = snapshot(&j);

    let target = tempfile::tempdir().unwrap();
    let path = target.path().join("vault.db");
    let opened = Backup::from_bytes(&bytes)
        .unwrap()
        .open_with_password(PASSWORD)
        .unwrap();
    assert_eq!(opened.record_count(), 5); // tag, two entries, plan, settings
    let restored = Journal::open(opened.restore(&path).unwrap());
    assert_eq!(snapshot(&restored), before);
    assert_eq!(restored.vault().vault_id(), j.vault().vault_id());
    restored.lock().unwrap();

    // The restored vault keeps the password and the recovery key.
    assert!(vault::unlock(&path, PASSWORD).is_ok());
    assert!(vault::unlock_with_recovery(&path, &fx.phrase, "new password 1").is_ok());
}

#[test]
fn restore_refuses_to_overwrite_a_vault() {
    let (fx, j) = populated();
    let opened = Backup::from_bytes(&export(&j))
        .unwrap()
        .open_with_password(PASSWORD)
        .unwrap();
    j.lock().unwrap();
    assert_eq!(
        opened.restore(&fx.path).unwrap_err(),
        BackupError::Vault(vault::VaultError::AlreadyExists)
    );
}

#[test]
fn replace_import_into_another_vault() {
    let (_src, j) = populated();
    let before = snapshot(&j);
    let (other_fx, mut other) = create("another password");
    other.quick_entry(EntryKindTag::Note, None, NOW).unwrap();
    other.create_tag("Sen", "#5b7f95").unwrap();

    let records = Backup::from_bytes(&export(&j))
        .unwrap()
        .open_with_password(PASSWORD)
        .unwrap();
    let report = other
        .import(
            records.decrypt().unwrap(),
            &|id| records.open_media(id.as_bytes()).unwrap(),
            ImportMode::Replace,
        )
        .unwrap();
    assert_eq!(
        report,
        ImportReport {
            added: 3,
            updated: 0,
            unchanged: 0,
            skipped: 0
        }
    );
    assert_eq!(snapshot(&other), before);

    // Re-encrypted with the target vault's key: still opens with its own password.
    other.lock().unwrap();
    let other = Journal::open(vault::unlock(&other_fx.path, "another password").unwrap());
    assert_eq!(snapshot(&other), before);
    assert_eq!(other.vault().unreadable_records(), 0);
}

#[test]
fn merge_keeps_newer_edits_and_maps_tags_by_name() {
    let (_src, mut source) = create(PASSWORD);
    let (target_fx, mut target) = create("target password");

    // Same tag name, different ids.
    let src_tag = source.create_tag("praca", "#4f6b47").unwrap().id;
    let our_tag = target.create_tag("Praca", "#8a6a2f").unwrap().id;

    // Entry A exists in both (copied via a first merge), then edited in the source later.
    let mut a = Entry::new(EntryKind::Note, NOW).unwrap();
    a.note = "A v1".into();
    let a_id = source.save_entry(a, NOW).unwrap().id;
    let first = Backup::from_bytes(&export(&source))
        .unwrap()
        .open_with_password(PASSWORD)
        .unwrap();
    target
        .import(
            first.decrypt().unwrap(),
            &|id| first.open_media(id.as_bytes()).unwrap(),
            ImportMode::Merge,
        )
        .unwrap();
    let mut a2 = source.entry(&a_id).unwrap().clone();
    a2.note = "A v2".into();
    a2.tags = vec![src_tag];
    source.save_entry(a2, NOW + Duration::hours(1)).unwrap();

    // Entry B exists only in the target; C only in the source.
    let b_id = target
        .quick_entry(EntryKindTag::Fear, Some(3), NOW)
        .unwrap()
        .id;
    let mut c = Entry::new(EntryKind::Note, NOW).unwrap();
    c.note = "C".into();
    let c_id = source.save_entry(c, NOW).unwrap().id;
    let mut plan = SafetyPlan::default();
    plan.self_coping = vec!["from the backup".into()];
    source.save_plan(plan).unwrap();

    let opened = Backup::from_bytes(&export(&source))
        .unwrap()
        .open_with_password(PASSWORD)
        .unwrap();
    let report = target
        .import(
            opened.decrypt().unwrap(),
            &|id| opened.open_media(id.as_bytes()).unwrap(),
            ImportMode::Merge,
        )
        .unwrap();
    assert_eq!(
        report,
        ImportReport {
            added: 1,
            updated: 1,
            unchanged: 1,
            skipped: 0
        }
    );

    target.lock().unwrap();
    let target = Journal::open(vault::unlock(&target_fx.path, "target password").unwrap());
    assert_eq!(target.entry(&a_id).unwrap().note, "A v2");
    assert_eq!(
        target.entry(&a_id).unwrap().tags,
        vec![our_tag],
        "mapped onto the existing tag"
    );
    assert!(target.entry(&b_id).is_some());
    assert_eq!(target.entry(&c_id).unwrap().note, "C");
    assert_eq!(target.tags().len(), 1);
    assert!(
        target.plan().self_coping.is_empty(),
        "merge keeps the current plan"
    );

    // Importing the same backup again changes nothing.
    let mut target = target;
    let again = Backup::from_bytes(&export(&source))
        .unwrap()
        .open_with_password(PASSWORD)
        .unwrap();
    let report = target
        .import(
            again.decrypt().unwrap(),
            &|id| again.open_media(id.as_bytes()).unwrap(),
            ImportMode::Merge,
        )
        .unwrap();
    assert_eq!(
        report,
        ImportReport {
            added: 0,
            updated: 0,
            unchanged: 3,
            skipped: 0
        }
    );
}

#[test]
fn opening_needs_the_right_key() {
    let (fx, j) = populated();
    let bytes = export(&j);
    let backup = || Backup::from_bytes(&bytes).unwrap();
    assert_eq!(
        backup().open_with_password("wrong password").unwrap_err(),
        BackupError::WrongPassword
    );
    assert!(backup().open_with_recovery(&fx.phrase).is_ok());
    let (other, _) = create(PASSWORD);
    assert_eq!(
        backup().open_with_recovery(&other.phrase).unwrap_err(),
        BackupError::WrongRecoveryKey
    );
    assert_eq!(
        backup().open_with_recovery("not a phrase").unwrap_err(),
        BackupError::InvalidRecoveryPhrase
    );
}

#[test]
fn old_backup_keeps_the_old_password() {
    // Decision D-9: a backup holds the DEK wrapped under the password of its time.
    let (_fx, mut j) = populated();
    let bytes = export(&j);
    j.vault_mut()
        .change_password(PASSWORD, "a newer password")
        .unwrap();
    assert!(
        Backup::from_bytes(&bytes)
            .unwrap()
            .open_with_password(PASSWORD)
            .is_ok()
    );
    assert_eq!(
        Backup::from_bytes(&bytes)
            .unwrap()
            .open_with_password("a newer password")
            .unwrap_err(),
        BackupError::WrongPassword
    );
}

#[test]
fn tampering_is_detected() {
    let (_fx, j) = populated();
    let backup = j.vault().export_backup().unwrap();
    let open = |b: Backup| b.open_with_password(PASSWORD).map(|_| ()).unwrap_err();

    // A flipped byte inside a record.
    let mut flipped = backup.clone();
    flipped.records[0].blob[30] ^= 1;
    assert_eq!(open(flipped), BackupError::Corrupted);
    // A record removed, or two records swapped.
    let mut removed = backup.clone();
    removed.records.pop();
    assert_eq!(open(removed), BackupError::Corrupted);
    let mut swapped = backup.clone();
    swapped.records.swap(0, 1);
    assert_eq!(open(swapped), BackupError::Corrupted);
    // The manifest of another backup.
    let (_other_fx, other) = populated();
    let mut foreign = backup.clone();
    foreign.records = other.vault().export_backup().unwrap().records;
    assert_eq!(open(foreign), BackupError::Corrupted);

    // Byte-level damage to the file.
    let bytes = backup.to_bytes().unwrap();
    assert_eq!(
        Backup::from_bytes(b"NOTMOSSB\x01\x00").unwrap_err(),
        BackupError::NotABackup
    );
    assert_eq!(
        Backup::from_bytes(&bytes[..bytes.len() / 2]).unwrap_err(),
        BackupError::Corrupted
    );
    let mut v3 = bytes.clone();
    v3[8] = 3;
    assert_eq!(
        Backup::from_bytes(&v3).unwrap_err(),
        BackupError::UnsupportedVersion
    );
    // A version 2 body labelled as version 1.
    let mut v1 = bytes.clone();
    v1[8] = 1;
    assert_eq!(
        Backup::from_bytes(&v1).unwrap_err(),
        BackupError::UnsupportedVersion
    );
    let mut garbled = bytes.clone();
    let mid = garbled.len() / 2;
    garbled[mid] ^= 0xff;
    let result =
        Backup::from_bytes(&garbled).and_then(|b| b.open_with_password(PASSWORD).map(|_| ()));
    assert!(result.is_err());
}

#[test]
fn round_trips_through_bytes() {
    let (_fx, j) = populated();
    let backup = j.vault().export_backup().unwrap();
    assert_eq!(
        Backup::from_bytes(&backup.to_bytes().unwrap()).unwrap(),
        backup
    );
}

// ---- format stability ----

/// A backup written by format version 1 (Mossbook 0.1–0.3); every later build
/// must import it. Frozen: this build writes version 2 and cannot regenerate it.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/backup-v1.mossbook"
);
const FIXTURE_PASSWORD: &str = "fixture password v1";

#[test]
fn imports_format_v1_fixture() {
    let bytes = std::fs::read(FIXTURE).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let opened = Backup::from_bytes(&bytes)
        .unwrap()
        .open_with_password(FIXTURE_PASSWORD)
        .unwrap();
    let j = Journal::open(opened.restore(&dir.path().join("vault.db")).unwrap());
    assert_eq!(j.vault().unreadable_records(), 0);
    let entries = j.entries(&EntryFilter::default());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].note, "fixture entry");
    assert_eq!(entries[0].occurred_at, Timestamp(NOW));
    assert_eq!(j.tags()[0].name, "fixture tag");
    assert_eq!(j.settings().autolock, AutoLock::After15Min);
}

/// A backup written by format version 2, with a recording (E-18, D-21).
/// Regenerate only on purpose: `cargo test -p mossbook-core --test backup -- --ignored`.
const FIXTURE_V2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/backup-v2.mossbook"
);
const FIXTURE_V2_PASSWORD: &str = "fixture password v2";

#[test]
fn imports_format_v2_fixture() {
    let bytes = std::fs::read(FIXTURE_V2).unwrap();
    assert_eq!(&bytes[8..10], &2u16.to_le_bytes());
    let dir = tempfile::tempdir().unwrap();
    let opened = Backup::from_bytes(&bytes)
        .unwrap()
        .open_with_password(FIXTURE_V2_PASSWORD)
        .unwrap();
    let j = Journal::open(opened.restore(&dir.path().join("vault.db")).unwrap());
    assert_eq!(j.vault().unreadable_records(), 0);
    let entries = j.entries(&EntryFilter::default());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].note, "fixture voice note");
    let voice = entries[0].voice.clone().unwrap();
    assert_eq!(voice.duration_ms, 200);
    assert_eq!(
        j.voice(&voice.id).unwrap().as_slice(),
        common::ogg_opus(10, 42).as_slice()
    );
}

#[test]
#[ignore = "writes tests/fixtures/backup-v2.mossbook"]
fn generate_backup_v2_fixture() {
    let (_fx, mut j) = create(FIXTURE_V2_PASSWORD);
    let mut e = Entry::new(EntryKind::Note, NOW).unwrap();
    e.note = "fixture voice note".into();
    e.voice = Some(j.save_voice(&common::ogg_opus(10, 42)).unwrap());
    j.save_entry(e, NOW).unwrap();
    std::fs::write(Path::new(FIXTURE_V2), export(&j)).unwrap();
}

// ---- voice notes (E-18, D-21) ----

fn with_voice(j: &mut Journal, seed: u8) -> Entry {
    let mut e = Entry::new(EntryKind::Note, NOW).unwrap();
    e.note = format!("voice {seed}");
    e.voice = Some(j.save_voice(&common::ogg_opus(10, seed)).unwrap());
    j.save_entry(e, NOW).unwrap().clone()
}

#[test]
fn recordings_travel_with_the_backup() {
    let (_fx, mut j) = populated();
    let e = with_voice(&mut j, 1);
    let voice = e.voice.clone().unwrap();
    let backup = j.vault().export_backup().unwrap();
    assert_eq!(backup.media.len(), 1);

    // Restore: the same recording, byte for byte.
    let dir = tempfile::tempdir().unwrap();
    let restored = Journal::open(
        Backup::from_bytes(&backup.to_bytes().unwrap())
            .unwrap()
            .open_with_password(PASSWORD)
            .unwrap()
            .restore(&dir.path().join("vault.db"))
            .unwrap(),
    );
    assert_eq!(
        restored.voice(&voice.id).unwrap().as_slice(),
        common::ogg_opus(10, 1).as_slice()
    );

    // Import into another journal: re-encrypted with that journal's key.
    let (_other_fx, mut other) = create("another password");
    let own = with_voice(&mut other, 2);
    let opened = backup.clone().open_with_password(PASSWORD).unwrap();
    other
        .import(
            opened.decrypt().unwrap(),
            &|id| opened.open_media(id.as_bytes()).unwrap(),
            ImportMode::Merge,
        )
        .unwrap();
    assert_eq!(
        other.voice(&voice.id).unwrap().as_slice(),
        common::ogg_opus(10, 1).as_slice()
    );
    assert!(other.voice(&own.voice.clone().unwrap().id).is_ok());
    let copied = other.vault().export_backup().unwrap();
    let blob = |b: &Backup| {
        b.media
            .iter()
            .find(|m| m.id == *voice.id.as_bytes())
            .unwrap()
            .blob
            .clone()
    };
    assert_ne!(blob(&copied), blob(&backup), "re-encrypted, not copied");

    // Replace: our own recording goes with our own entries.
    other
        .import(
            opened.decrypt().unwrap(),
            &|id| opened.open_media(id.as_bytes()).unwrap(),
            ImportMode::Replace,
        )
        .unwrap();
    assert_eq!(
        other.vault().media_ids().unwrap(),
        vec![*voice.id.as_bytes()]
    );
}

#[test]
fn an_entry_whose_recording_is_missing_keeps_its_text() {
    let (_fx, mut j) = populated();
    let e = with_voice(&mut j, 1);
    let opened = j
        .vault()
        .export_backup()
        .unwrap()
        .open_with_password(PASSWORD)
        .unwrap();
    let (_other_fx, mut other) = create("another password");
    other
        .import(opened.decrypt().unwrap(), &|_| None, ImportMode::Merge)
        .unwrap();
    let imported = other.entry(&e.id).unwrap();
    assert_eq!(imported.note, "voice 1");
    assert_eq!(imported.voice, None);
    // A recording that is not a valid file is dropped the same way.
    let (_third_fx, mut third) = create("third password");
    third
        .import(
            opened.decrypt().unwrap(),
            &|_| Some(zeroize::Zeroizing::new(b"not ogg".to_vec())),
            ImportMode::Merge,
        )
        .unwrap();
    assert_eq!(third.entry(&e.id).unwrap().voice, None);
    assert!(third.vault().media_ids().unwrap().is_empty());
}

#[test]
fn tampering_with_recordings_is_detected() {
    let (_fx, mut j) = populated();
    with_voice(&mut j, 1);
    with_voice(&mut j, 2);
    let backup = j.vault().export_backup().unwrap();
    let open = |b: Backup| b.open_with_password(PASSWORD).map(|_| ()).unwrap_err();

    let mut flipped = backup.clone();
    flipped.media[0].blob[40] ^= 1;
    assert_eq!(open(flipped), BackupError::Corrupted);
    let mut removed = backup.clone();
    removed.media.pop();
    assert_eq!(open(removed), BackupError::Corrupted);
    let mut swapped = backup.clone();
    swapped.media.swap(0, 1);
    assert_eq!(open(swapped), BackupError::Corrupted);
    // A recording passed off as a record, or the other way round.
    let mut moved = backup.clone();
    let m = moved.media.pop().unwrap();
    moved.records.push(m);
    assert_eq!(
        Backup::from_bytes(&moved.to_bytes().unwrap()).unwrap_err(),
        BackupError::Corrupted
    );
    let mut moved = backup.clone();
    let r = moved.records.pop().unwrap();
    moved.media.push(r);
    assert_eq!(
        Backup::from_bytes(&moved.to_bytes().unwrap()).unwrap_err(),
        BackupError::Corrupted
    );
}

#[test]
fn a_version_2_backup_cannot_pass_as_version_1_without_its_recordings() {
    use ciborium::Value;
    let (_fx, mut j) = populated();
    with_voice(&mut j, 1);
    let bytes = export(&j);
    let mut body: Value = ciborium::from_reader(&bytes[10..]).unwrap();
    let Value::Map(fields) = &mut body else {
        panic!("body is a map")
    };
    fields.retain(|(k, _)| k.as_text() != Some("media"));
    for (k, v) in fields.iter_mut() {
        if k.as_text() == Some("vault_format") {
            *v = Value::Integer(1.into());
        }
    }
    let mut forged = bytes[..8].to_vec();
    forged.extend_from_slice(&1u16.to_le_bytes());
    ciborium::into_writer(&body, &mut forged).unwrap();
    let backup = Backup::from_bytes(&forged).expect("well-formed version 1");
    assert_eq!(
        backup.open_with_password(PASSWORD).unwrap_err(),
        BackupError::Corrupted
    );
}

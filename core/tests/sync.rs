//! Sync between the user's own devices (D-22): two copies of one journal, each
//! merging the other's sync file. Synthetic data only.

mod common;

use std::path::PathBuf;

use mossbook_core::backup::{Backup, BackupError};
use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::domain::{
    AutoLock, Entry, EntryKind, Id, SafetyPlan, Settings, Timestamp, encode,
};
use mossbook_core::journal::{Journal, SyncReport};
use mossbook_core::query::EntryFilter;
use mossbook_core::vault::{self, PendingVault};
use tempfile::TempDir;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

const PASSWORD: &str = "correct horse battery";
const T0: OffsetDateTime = datetime!(2026-10-22 10:00 +02:00);

fn at(minutes: i64) -> OffsetDateTime {
    T0 + Duration::minutes(minutes)
}

struct Device {
    _dir: TempDir,
    path: PathBuf,
    journal: Journal,
}

/// The first device, and a second one holding the same journal (as after pairing).
fn two_devices() -> (Device, Device) {
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
    let a = Journal::open(pending.write(&path).unwrap());

    let dir_b = tempfile::tempdir().unwrap();
    let path_b = dir_b.path().join("vault.db");
    let b = Journal::open(
        a.vault()
            .export_backup()
            .unwrap()
            .open_with_password(PASSWORD)
            .unwrap()
            .restore(&path_b)
            .unwrap(),
    );
    (
        Device {
            _dir: dir,
            path,
            journal: a,
        },
        Device {
            _dir: dir_b,
            path: path_b,
            journal: b,
        },
    )
}

fn sync_file(from: &Device) -> Vec<u8> {
    from.journal
        .vault()
        .export_backup()
        .unwrap()
        .to_bytes()
        .unwrap()
}

/// What a device does with another device's file.
fn receive(to: &mut Device, file: &[u8], now: OffsetDateTime) -> Result<SyncReport, BackupError> {
    let opened = to
        .journal
        .vault()
        .open_sync_file(Backup::from_bytes(file)?)?;
    to.journal.vault_mut().adopt_password_from(&opened).unwrap();
    let records = opened.decrypt()?;
    Ok(to
        .journal
        .merge_from_device(
            records,
            &|id| opened.open_media(id.as_bytes()).ok().flatten(),
            now,
        )
        .unwrap())
}

fn both_ways(a: &mut Device, b: &mut Device, now: OffsetDateTime) {
    let from_a = sync_file(a);
    receive(b, &from_a, now).unwrap();
    let from_b = sync_file(b);
    receive(a, &from_b, now).unwrap();
}

fn note(j: &mut Journal, text: &str, now: OffsetDateTime) -> Id {
    let mut e = Entry::new(EntryKind::Note, now).unwrap();
    e.note = text.into();
    j.save_entry(e, now).unwrap().id
}

/// Everything the user sees, in a stable order, to compare two devices.
fn content(j: &Journal) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = j
        .entries(&EntryFilter::default())
        .into_iter()
        .map(|e| encode(e).unwrap().to_vec())
        .collect();
    out.extend(j.tags().into_iter().map(|t| encode(t).unwrap().to_vec()));
    out.push(encode(j.plan()).unwrap().to_vec());
    let mut settings = j.settings().clone();
    settings.unsaved_entry = None;
    out.push(encode(&settings).unwrap().to_vec());
    out.sort();
    out
}

#[test]
fn new_entries_and_recordings_reach_the_other_device() {
    let (mut a, mut b) = two_devices();
    note(&mut a.journal, "written on the phone", at(1));
    let mut spoken = Entry::new(EntryKind::Note, at(2)).unwrap();
    spoken.voice = Some(b.journal.save_voice(&common::ogg_opus(20, 5)).unwrap());
    let spoken = b.journal.save_entry(spoken, at(2)).unwrap().clone();

    both_ways(&mut a, &mut b, at(3));
    assert_eq!(a.journal.entries(&EntryFilter::default()).len(), 2);
    assert_eq!(content(&a.journal), content(&b.journal));
    let voice = spoken.voice.clone().unwrap().id;
    assert_eq!(
        a.journal.voice(&voice).unwrap().as_slice(),
        common::ogg_opus(20, 5).as_slice()
    );
}

#[test]
fn the_newer_edit_wins_whichever_file_comes_first() {
    for a_first in [true, false] {
        let (mut a, mut b) = two_devices();
        let id = note(&mut a.journal, "first", at(1));
        both_ways(&mut a, &mut b, at(2));

        let mut on_a = a.journal.entry(&id).unwrap().clone();
        on_a.note = "edited on the phone".into();
        a.journal.save_entry(on_a, at(5)).unwrap();
        let mut on_b = b.journal.entry(&id).unwrap().clone();
        on_b.note = "edited on the computer, later".into();
        b.journal.save_entry(on_b, at(6)).unwrap();

        let (from_a, from_b) = (sync_file(&a), sync_file(&b));
        if a_first {
            receive(&mut b, &from_a, at(7)).unwrap();
            receive(&mut a, &from_b, at(7)).unwrap();
        } else {
            receive(&mut a, &from_b, at(7)).unwrap();
            receive(&mut b, &from_a, at(7)).unwrap();
        }
        assert_eq!(
            a.journal.entry(&id).unwrap().note,
            "edited on the computer, later"
        );
        assert_eq!(content(&a.journal), content(&b.journal));
    }
}

#[test]
fn deletions_travel_and_a_later_edit_brings_an_entry_back() {
    let (mut a, mut b) = two_devices();
    let gone = note(&mut a.journal, "to delete", at(1));
    let back = note(&mut a.journal, "to delete, then edit", at(1));
    both_ways(&mut a, &mut b, at(2));

    a.journal.delete_entry(&gone, at(5)).unwrap();
    a.journal.delete_entry(&back, at(5)).unwrap();
    // On the other device, before it heard of the deletion: one older edit,
    // one newer.
    let mut older = b.journal.entry(&gone).unwrap().clone();
    older.note = "edited before the deletion".into();
    b.journal.save_entry(older, at(4)).unwrap();
    let mut newer = b.journal.entry(&back).unwrap().clone();
    newer.note = "edited after the deletion".into();
    b.journal.save_entry(newer, at(6)).unwrap();

    both_ways(&mut a, &mut b, at(7));
    for j in [&a.journal, &b.journal] {
        assert!(
            j.entry(&gone).is_none(),
            "the deletion wins over an older edit"
        );
        assert_eq!(j.entry(&back).unwrap().note, "edited after the deletion");
    }
    assert_eq!(content(&a.journal), content(&b.journal));

    // The deletion is not lost when a device restarts.
    let path = a.path.clone();
    a.journal.lock().unwrap();
    a.journal = Journal::open(vault::unlock(&path, PASSWORD).unwrap());
    let report = receive(&mut a, &sync_file(&b), at(8)).unwrap();
    assert_eq!(report, SyncReport::default(), "nothing to do twice");
    assert!(a.journal.entry(&gone).is_none());
}

#[test]
fn tags_with_the_same_name_become_one_and_deleted_tags_go() {
    let (mut a, mut b) = two_devices();
    let work_a = a.journal.create_tag("Praca", "#4f6b47", at(1)).unwrap().id;
    let work_b = b.journal.create_tag("praca", "#000000", at(2)).unwrap().id;
    let mut e = Entry::new(EntryKind::Note, at(3)).unwrap();
    e.tags = vec![work_b];
    let tagged = b.journal.save_entry(e, at(3)).unwrap().id;
    let sleep = a.journal.create_tag("Sen", "#123456", at(1)).unwrap().id;

    both_ways(&mut a, &mut b, at(4));
    both_ways(&mut a, &mut b, at(5));
    let keep = work_a.min(work_b);
    for j in [&a.journal, &b.journal] {
        let names: Vec<_> = j.tags().iter().map(|t| t.name.to_lowercase()).collect();
        assert_eq!(names, vec!["praca", "sen"]);
        assert_eq!(j.entry(&tagged).unwrap().tags, vec![keep]);
    }
    assert_eq!(content(&a.journal), content(&b.journal));

    a.journal.delete_tag(&sleep, at(6)).unwrap();
    both_ways(&mut a, &mut b, at(7));
    assert!(b.journal.tags().iter().all(|t| t.id != sleep));
}

#[test]
fn plan_and_settings_follow_the_newer_change_but_the_open_form_stays() {
    let (mut a, mut b) = two_devices();
    let mut plan = SafetyPlan::default();
    plan.self_coping = vec!["synthetic walk".into()];
    a.journal.save_plan(plan, at(1)).unwrap();
    let mut settings = Settings::default();
    settings.autolock = AutoLock::After5Min;
    b.journal.save_settings(settings, at(2)).unwrap();
    let draft = serde_json::from_str(r#"{ "kind": "note", "note": "half written" }"#).unwrap();
    b.journal.keep_unsaved_entry(draft, at(3)).unwrap();

    both_ways(&mut a, &mut b, at(4));
    for j in [&a.journal, &b.journal] {
        assert_eq!(j.plan().self_coping, vec!["synthetic walk".to_string()]);
        assert_eq!(j.settings().autolock, AutoLock::After5Min);
    }
    assert!(
        a.journal.unsaved_entry().is_none(),
        "a form stays on its device"
    );
    assert!(b.journal.unsaved_entry().is_some());
}

#[test]
fn a_password_change_reaches_the_other_device_and_an_old_file_cannot_undo_it() {
    let (mut a, mut b) = two_devices();
    let old_file = sync_file(&a);
    a.journal
        .vault_mut()
        .change_password(PASSWORD, "a newer synthetic password")
        .unwrap();
    receive(&mut b, &sync_file(&a), at(1)).unwrap();
    // An older file of the same device, replayed, does not bring the old one back.
    receive(&mut b, &old_file, at(2)).unwrap();

    let path = b.path.clone();
    b.journal.lock().unwrap();
    assert!(vault::unlock(&path, PASSWORD).is_err());
    assert!(vault::unlock(&path, "a newer synthetic password").is_ok());
}

#[test]
fn files_of_another_journal_or_damaged_ones_are_refused() {
    let (mut a, _b) = two_devices();
    let (other, _) = two_devices();
    assert_eq!(
        receive(&mut a, &sync_file(&other), at(1)).unwrap_err(),
        BackupError::OtherJournal
    );

    let (mut a, mut b) = two_devices();
    note(&mut b.journal, "something to tamper with", at(1));
    let mut file = sync_file(&b);
    let mid = file.len() / 2;
    file[mid] ^= 0x01;
    assert!(receive(&mut a, &file, at(1)).is_err());
    // A file with a record taken out.
    let mut backup = b.journal.vault().export_backup().unwrap();
    backup.records.pop();
    assert_eq!(
        receive(&mut a, &backup.to_bytes().unwrap(), at(1)).unwrap_err(),
        BackupError::Corrupted
    );
}

#[test]
fn ties_end_the_same_on_both_devices() {
    let (mut a, mut b) = two_devices();
    let id = note(&mut a.journal, "same minute", at(1));
    both_ways(&mut a, &mut b, at(2));
    let mut on_a = a.journal.entry(&id).unwrap().clone();
    on_a.note = "A at 10:05".into();
    a.journal.save_entry(on_a, at(5)).unwrap();
    let mut on_b = b.journal.entry(&id).unwrap().clone();
    on_b.note = "B at 10:05".into();
    b.journal.save_entry(on_b, at(5)).unwrap();

    let (from_a, from_b) = (sync_file(&a), sync_file(&b));
    receive(&mut b, &from_a, at(6)).unwrap();
    receive(&mut a, &from_b, at(6)).unwrap();
    assert_eq!(content(&a.journal), content(&b.journal));
    assert_eq!(a.journal.entry(&id).unwrap().updated_at, Timestamp(at(5)));
}

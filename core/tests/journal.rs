//! Integration tests for the journal on a real vault file. All data is synthetic.

mod common;

use std::path::PathBuf;

use mossbook_core::crypto::aead::RecordType;
use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::domain::DomainError;
use mossbook_core::domain::{
    AutoLock, Contact, Emotion, Entry, EntryKind, EntryKindTag, Id, SafetyPlan, Settings, Symptom,
    Timestamp, encode,
};
use mossbook_core::journal::{Journal, JournalError};
use mossbook_core::query::{EntryFilter, StatsRange};
use mossbook_core::vault::{self, PendingVault};
use proptest::prelude::*;
use tempfile::TempDir;
use time::macros::datetime;
use time::{Duration, OffsetDateTime, UtcOffset};

const PASSWORD: &str = "correct horse battery";
const NOW: OffsetDateTime = datetime!(2026-10-19 20:00 +02:00);

struct Fixture {
    _dir: TempDir,
    path: PathBuf,
}

fn create() -> (Fixture, Journal) {
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
    let journal = Journal::open(pending.write(&path).unwrap());
    (Fixture { _dir: dir, path }, journal)
}

fn reopen(fx: &Fixture, journal: Journal) -> Journal {
    journal.lock().unwrap();
    Journal::open(vault::unlock(&fx.path, PASSWORD).unwrap())
}

#[test]
fn entries_persist_and_keep_created_at() {
    let (fx, mut j) = create();
    let mut e = Entry::new(EntryKind::empty(EntryKindTag::Event), NOW).unwrap();
    e.note = "synthetic event".into();
    let id = j.save_entry(e, NOW).unwrap().id;

    let mut edited = j.entry(&id).unwrap().clone();
    edited.note = "synthetic event, edited".into();
    edited.created_at = Timestamp(NOW + Duration::days(5)); // must be ignored
    j.save_entry(edited, NOW + Duration::hours(1)).unwrap();

    let j = reopen(&fx, j);
    let e = j.entry(&id).unwrap();
    assert_eq!(e.note, "synthetic event, edited");
    assert_eq!(e.created_at, Timestamp(NOW));
    assert_eq!(e.updated_at, Timestamp(NOW + Duration::hours(1)));
    assert_eq!(j.vault().unreadable_records(), 0);
}

#[test]
fn invalid_entries_are_not_saved() {
    let (fx, mut j) = create();
    let mut e = Entry::new(EntryKind::Note, NOW).unwrap();
    e.mood = Some(9);
    assert_eq!(
        j.save_entry(e, NOW).unwrap_err(),
        JournalError::Domain(DomainError::InvalidField("mood"))
    );
    let mut e = Entry::new(EntryKind::Note, NOW).unwrap();
    e.tags = vec![Id::from_bytes([9; 16])];
    assert_eq!(
        j.save_entry(e, NOW).unwrap_err(),
        JournalError::Domain(DomainError::InvalidField("tags"))
    );
    let j = reopen(&fx, j);
    assert!(j.entries(&EntryFilter::default()).is_empty());
}

#[test]
fn quick_entry_and_delete() {
    let (fx, mut j) = create();
    let fear = j.quick_entry(EntryKindTag::Fear, Some(7), NOW).unwrap().id;
    let mood = j.quick_entry(EntryKindTag::Mood, Some(2), NOW).unwrap().id;
    assert_eq!(j.entry(&fear).unwrap().intensity, Some(7));
    assert_eq!(j.entry(&mood).unwrap().mood, Some(2));
    assert!(j.entry(&mood).unwrap().incomplete);
    assert!(j.quick_entry(EntryKindTag::Mood, Some(6), NOW).is_err());
    let nightmare = j.quick_entry(EntryKindTag::Nightmare, None, NOW).unwrap();
    assert!(nightmare.sensitive, "nightmares are hidden by default");
    let nightmare = nightmare.id;
    assert!(j.delete_entry(&nightmare, NOW).unwrap());

    assert!(j.delete_entry(&fear, NOW).unwrap());
    assert!(!j.delete_entry(&fear, NOW).unwrap());
    let j = reopen(&fx, j);
    let incomplete = j.entries(&EntryFilter {
        incomplete_only: true,
        ..Default::default()
    });
    assert_eq!(
        incomplete.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![mood]
    );
}

#[test]
fn tags_rename_delete_and_merge() {
    let (fx, mut j) = create();
    let work = j.create_tag("Praca", "#336699", NOW).unwrap().id;
    let job = j.create_tag("robota", "#336699", NOW).unwrap().id;
    let sleep = j.create_tag("sen", "#224466", NOW).unwrap().id;
    assert_eq!(
        j.create_tag(" praca ", "#000000", NOW).unwrap_err(),
        JournalError::DuplicateTagName
    );

    let mut renamed = j
        .tags()
        .into_iter()
        .find(|t| t.id == sleep)
        .unwrap()
        .clone();
    renamed.name = "Sen".into();
    j.save_tag(renamed, NOW).unwrap();

    let mut e1 = Entry::new(EntryKind::Note, NOW).unwrap();
    e1.tags = vec![work, sleep, work];
    let e1 = j.save_entry(e1, NOW).unwrap().id;
    let mut e2 = Entry::new(EntryKind::Note, NOW).unwrap();
    e2.tags = vec![job, work];
    let e2 = j.save_entry(e2, NOW).unwrap().id;
    assert_eq!(
        j.entry(&e1).unwrap().tags,
        vec![work, sleep],
        "duplicates dropped"
    );

    j.merge_tags(&job, &work, NOW).unwrap();
    assert!(j.delete_tag(&sleep, NOW).unwrap());
    assert!(!j.delete_tag(&sleep, NOW).unwrap());
    assert!(j.merge_tags(&work, &work, NOW).is_err());

    let j = reopen(&fx, j);
    let names: Vec<&str> = j.tags().iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["Praca"]);
    assert_eq!(j.entry(&e1).unwrap().tags, vec![work]);
    assert_eq!(j.entry(&e2).unwrap().tags, vec![work]);
    assert_eq!(j.vault().unreadable_records(), 0);
}

#[test]
fn plan_and_settings_persist_with_defaults() {
    let (fx, mut j) = create();
    assert_eq!(j.settings(), &Settings::default());
    assert_eq!(j.plan(), &SafetyPlan::default());

    let mut plan = SafetyPlan::default();
    plan.self_coping = vec!["synthetic walk".into(), " ".into()];
    let mut contact = Contact::default();
    contact.name = "Synthetic Friend".into();
    contact.phone = "+48 000 000 000".into();
    plan.contacts = vec![contact];
    j.save_plan(plan, NOW).unwrap();

    let mut settings = Settings::default();
    settings.autolock = AutoLock::After5Min;
    settings.locale = Some("uk".into());
    j.save_settings(settings.clone(), NOW).unwrap();
    let mut bad = settings.clone();
    bad.locale = Some("xx".into());
    assert!(j.save_settings(bad, NOW).is_err());

    let j = reopen(&fx, j);
    assert_eq!(j.plan().self_coping, vec!["synthetic walk".to_string()]);
    assert_eq!(j.plan().contacts[0].name, "Synthetic Friend");
    // The journal stamps the change for sync (D-22); the UI cannot set it.
    assert_eq!(j.plan().updated_at, Some(Timestamp(NOW)));
    settings.updated_at = Some(Timestamp(NOW));
    assert_eq!(j.settings(), &settings);
}

#[test]
fn last_export_is_set_only_by_the_journal() {
    let (fx, mut j) = create();
    assert_eq!(j.settings().last_export, None);
    j.record_export(NOW).unwrap();
    assert_eq!(j.settings().last_export, Some(Timestamp(NOW)));
    // The UI sends settings without (or with a stale) last_export: it is kept.
    let mut settings = j.settings().clone();
    settings.last_export = None;
    settings.auto_backup = false;
    j.save_settings(settings, NOW).unwrap();
    let j = reopen(&fx, j);
    assert_eq!(j.settings().last_export, Some(Timestamp(NOW)));
    assert!(!j.settings().auto_backup);
}

#[test]
fn json_export_holds_the_whole_journal() {
    let (_fx, mut j) = create();
    let tag = j.create_tag("synthetic tag", "#2e7d4f", NOW).unwrap().id;
    let later = j.quick_entry(EntryKindTag::Fear, Some(7), NOW).unwrap().id;
    let earlier = j
        .quick_entry(EntryKindTag::Note, None, NOW - Duration::days(1))
        .unwrap()
        .id;
    let mut plan = SafetyPlan::default();
    plan.warning_signs = vec!["synthetic sign".into()];
    j.save_plan(plan, NOW).unwrap();

    let bytes = j.export_json(NOW).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["format"], "mossbook-export");
    assert_eq!(doc["version"], 4);
    assert_eq!(doc["exported_at"], "2026-10-19T20:00:00+02:00");
    let entries = doc["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    // Oldest first, and the entries read back as the app's own type.
    let first: Entry = serde_json::from_value(entries[0].clone()).unwrap();
    assert_eq!(first.id, earlier);
    assert_eq!(entries[1]["id"], serde_json::to_value(later).unwrap());
    assert_eq!(doc["tags"][0]["id"], serde_json::to_value(tag).unwrap());
    assert_eq!(doc["plan"]["warning_signs"][0], "synthetic sign");
    assert!(doc["settings"]["auto_backup"].as_bool().unwrap());
}

#[test]
fn record_under_wrong_id_is_unreadable() {
    let (fx, mut j) = create();
    let e = Entry::new(EntryKind::Note, NOW).unwrap();
    // A valid entry stored under another row id (e.g. a copied blob that was re-encrypted).
    j.vault_mut()
        .put([7; 16], RecordType::Entry, &encode(&e).unwrap())
        .unwrap();
    j.vault_mut()
        .put([8; 16], RecordType::Entry, b"not cbor")
        .unwrap();
    j.vault_mut()
        .put(
            [9; 16],
            RecordType::Plan,
            &encode(&SafetyPlan::default()).unwrap(),
        )
        .unwrap();
    let j = reopen(&fx, j);
    assert_eq!(j.vault().unreadable_records(), 3);
    assert!(j.entries(&EntryFilter::default()).is_empty());
}

#[test]
fn mood_series_and_search_over_saved_entries() {
    let (fx, mut j) = create();
    for (days_ago, mood, note) in [
        (1, 2, "kiepski sen"),
        (1, 4, "spacer"),
        (3, 5, "Łódź, koncert"),
    ] {
        let mut e = Entry::new(EntryKind::empty(EntryKindTag::Mood), NOW).unwrap();
        e.occurred_at = Timestamp(NOW - Duration::days(days_ago));
        e.mood = Some(mood);
        e.note = note.into();
        j.save_entry(e, NOW).unwrap();
    }
    let j = reopen(&fx, j);
    let series = j.mood_series(StatsRange::Week, NOW);
    assert_eq!(
        series
            .iter()
            .map(|p| (p.average, p.count))
            .collect::<Vec<_>>(),
        vec![(5.0, 1), (3.0, 2)]
    );
    let found = j.entries(&EntryFilter {
        text: Some("lodz".into()),
        ..Default::default()
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].note, "Łódź, koncert");
}

// ---- property test: any entry survives encode → encrypt → disk → decrypt → decode ----

fn text() -> impl Strategy<Value = String> {
    // Arbitrary Unicode, including Polish/Ukrainian letters, emoji and control characters.
    proptest::string::string_regex("(?s).{0,40}").unwrap()
}

fn timestamp() -> impl Strategy<Value = Timestamp> {
    (
        0i64..4_000_000_000,
        0u32..1_000_000_000,
        -12i8..=14,
        prop_oneof![Just(0i8), Just(30), Just(45)],
    )
        .prop_map(|(secs, nanos, h, m)| {
            let m = if h < 0 { -m } else { m };
            let offset = UtcOffset::from_hms(h, m, 0).unwrap();
            let t = OffsetDateTime::from_unix_timestamp(secs)
                .unwrap()
                .replace_nanosecond(nanos)
                .unwrap();
            Timestamp(t.to_offset(offset))
        })
}

fn emotion() -> impl Strategy<Value = Emotion> {
    prop_oneof![
        Just(Emotion::Anxiety),
        Just(Emotion::Calm),
        Just(Emotion::Shame),
        text().prop_map(Emotion::Custom)
    ]
}

fn symptom() -> impl Strategy<Value = Symptom> {
    prop_oneof![
        Just(Symptom::RacingHeart),
        Just(Symptom::Unreality),
        text().prop_map(Symptom::Custom)
    ]
}

fn kind() -> impl Strategy<Value = EntryKind> {
    prop_oneof![
        (
            proptest::option::of(1u8..=5),
            proptest::collection::vec(emotion(), 0..4)
        )
            .prop_map(|(energy, emotions)| EntryKind::Mood { energy, emotions }),
        (text(), text(), text(), text(), text()).prop_map(
            |(what, place, body, thoughts, helped)| EntryKind::Event {
                what,
                place,
                body,
                thoughts,
                helped
            }
        ),
        (
            text(),
            proptest::collection::vec(symptom(), 0..4),
            proptest::option::of(any::<bool>()),
            proptest::option::of(text()),
            proptest::option::of(0u8..=10)
        )
            .prop_map(
                |(trigger, symptoms, avoidance, strategy, intensity_after)| EntryKind::Fear {
                    trigger,
                    symptoms,
                    avoidance,
                    strategy,
                    intensity_after
                }
            ),
        (
            proptest::option::of(timestamp()),
            proptest::option::of(timestamp()),
            proptest::option::of(1u8..=5),
            proptest::option::of(any::<u8>()),
            any::<bool>()
        )
            .prop_map(
                |(bedtime, wake, quality, awakenings, nightmare)| EntryKind::Sleep {
                    bedtime,
                    wake,
                    quality,
                    awakenings,
                    nightmare
                }
            ),
        (text(), text(), proptest::collection::vec(timestamp(), 0..3)).prop_map(
            |(content, rescripted, rehearsals)| EntryKind::Nightmare {
                content,
                rescripted,
                rehearsals
            }
        ),
        Just(EntryKind::Note),
        (
            text(),
            text(),
            proptest::collection::vec(any::<[u8; 16]>(), 0..3)
        )
            .prop_map(|(agreed, todo, ids)| EntryKind::Session {
                agreed,
                todo,
                discussed: ids.into_iter().map(Id::from_bytes).collect(),
            }),
    ]
}

fn entry() -> impl Strategy<Value = Entry> {
    (
        kind(),
        timestamp(),
        proptest::option::of(1u8..=5),
        proptest::option::of(0u8..=10),
        text(),
        any::<bool>(),
        any::<bool>(),
        any::<[u8; 16]>(),
        any::<bool>(),
    )
        .prop_map(
            |(kind, occurred_at, mood, intensity, note, incomplete, sensitive, id, discuss)| {
                // A session cannot be flagged for one.
                let discuss = discuss && kind.tag() != EntryKindTag::Session;
                let mut e = Entry::new(kind, NOW).unwrap();
                e.id = Id::from_bytes(id);
                e.occurred_at = occurred_at;
                e.mood = mood;
                e.intensity = intensity;
                e.note = note;
                e.incomplete = incomplete;
                e.sensitive = sensitive;
                e.discuss = discuss;
                e
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn any_entry_round_trips_through_the_vault(entries in proptest::collection::vec(entry(), 1..6)) {
        let (fx, mut j) = create();
        let mut saved = Vec::new();
        for e in entries {
            saved.push(j.save_entry(e, NOW).unwrap().clone());
        }
        let j = reopen(&fx, j);
        prop_assert_eq!(j.vault().unreadable_records(), 0);
        for e in &saved {
            let back = j.entry(&e.id).unwrap();
            prop_assert_eq!(back, e);
            // OffsetDateTime equality ignores the offset; check it separately.
            prop_assert_eq!(back.occurred_at.0.offset(), e.occurred_at.0.offset());
        }
    }
}

#[test]
fn drafts_create_and_edit_but_cannot_forge_metadata() {
    use mossbook_core::domain::EntryDraft;
    let (fx, mut j) = create();
    let json = r#"{ "kind": { "mood": { "energy": 3, "emotions": ["calm", { "custom": "synthetic" }] } },
                    "mood": 4, "note": "draft note",
                    "created_at": "2000-01-01T00:00:00Z", "schema": 99 }"#;
    let draft: EntryDraft = serde_json::from_str(json).unwrap();
    let created = j.save_draft(&draft, NOW).unwrap().clone();
    assert_eq!(created.created_at, Timestamp(NOW));
    assert_eq!(created.occurred_at, Timestamp(NOW));
    assert_eq!(created.schema, 1);

    let later = NOW + Duration::hours(2);
    let edit = format!(
        r#"{{ "id": "{}", "kind": "note", "note": "edited" }}"#,
        created.id
    );
    let draft: EntryDraft = serde_json::from_str(&edit).unwrap();
    let edited = j.save_draft(&draft, later).unwrap().clone();
    assert_eq!(edited.id, created.id);
    assert_eq!(edited.created_at, Timestamp(NOW));
    assert_eq!(
        edited.occurred_at,
        Timestamp(NOW),
        "kept when the draft has none"
    );
    assert_eq!(edited.kind, EntryKind::Note);
    assert_eq!(edited.mood, None);

    let unknown: EntryDraft =
        serde_json::from_str(r#"{ "id": "01890000-0000-7000-8000-000000000000", "kind": "note" }"#)
            .unwrap();
    assert_eq!(
        j.save_draft(&unknown, NOW).unwrap_err(),
        JournalError::Domain(DomainError::NotFound)
    );
    let j = reopen(&fx, j);
    assert_eq!(j.entries(&EntryFilter::default()).len(), 1);
}

fn unsaved_draft(note: &str) -> mossbook_core::domain::EntryDraft {
    serde_json::from_str(&format!(
        r#"{{ "kind": {{ "fear": {{ "trigger": "synthetic trigger" }} }}, "intensity": 6, "note": "{note}" }}"#
    ))
    .unwrap()
}

#[test]
fn unsaved_entry_survives_a_lock_until_the_form_is_saved() {
    let (fx, mut j) = create();
    assert!(j.unsaved_entry().is_none());
    j.keep_unsaved_entry(unsaved_draft("first words"), NOW)
        .unwrap();
    let later = NOW + Duration::minutes(1);
    j.keep_unsaved_entry(unsaved_draft("first words, more"), later)
        .unwrap();

    let mut j = reopen(&fx, j);
    let kept = j.unsaved_entry().expect("kept across the lock").clone();
    assert_eq!(kept.kept_at, Timestamp(later));
    assert_eq!(kept.draft, unsaved_draft("first words, more"));
    assert_eq!(
        j.entries(&EntryFilter::default()).len(),
        0,
        "not an entry yet"
    );

    let saved = j.save_draft(&kept.draft, later).unwrap().clone();
    assert_eq!(saved.note, "first words, more");
    assert!(
        j.unsaved_entry().is_none(),
        "saving the form forgets the copy"
    );
    let j = reopen(&fx, j);
    assert!(j.unsaved_entry().is_none());
    assert_eq!(j.entries(&EntryFilter::default()).len(), 1);
}

#[test]
fn unsaved_entry_can_be_discarded() {
    let (fx, mut j) = create();
    j.discard_unsaved_entry().unwrap();
    j.keep_unsaved_entry(unsaved_draft("synthetic"), NOW)
        .unwrap();
    j.discard_unsaved_entry().unwrap();
    assert!(j.unsaved_entry().is_none());
    let j = reopen(&fx, j);
    assert!(j.unsaved_entry().is_none());
    assert_eq!(j.entries(&EntryFilter::default()).len(), 0);
}

#[test]
fn a_failed_save_keeps_the_unsaved_entry() {
    let (_fx, mut j) = create();
    j.keep_unsaved_entry(unsaved_draft("synthetic"), NOW)
        .unwrap();
    let unknown: mossbook_core::domain::EntryDraft =
        serde_json::from_str(r#"{ "id": "01890000-0000-7000-8000-000000000000", "kind": "note" }"#)
            .unwrap();
    assert!(j.save_draft(&unknown, NOW).is_err());
    assert!(j.unsaved_entry().is_some());
}

#[test]
fn settings_from_the_ui_cannot_set_or_erase_the_unsaved_entry() {
    let (fx, mut j) = create();
    // A forged copy on a journal without one is ignored.
    let mut forged = j.settings().clone();
    forged.unsaved_entry = Some(mossbook_core::domain::UnsavedEntry {
        draft: unsaved_draft("forged"),
        kept_at: Timestamp(NOW),
    });
    j.save_settings(forged, NOW).unwrap();
    assert!(j.unsaved_entry().is_none());

    // Settings sent without it (or with a stale copy) keep the stored one.
    j.keep_unsaved_entry(unsaved_draft("real"), NOW).unwrap();
    let mut settings = j.settings().clone();
    settings.unsaved_entry = None;
    settings.auto_backup = false;
    j.save_settings(settings, NOW).unwrap();
    let j = reopen(&fx, j);
    assert_eq!(j.unsaved_entry().unwrap().draft, unsaved_draft("real"));
    assert!(!j.settings().auto_backup);
}

#[test]
fn settings_with_an_unsaved_entry_still_decode_in_older_builds() {
    // Older builds have no `unsaved_entry` field; unknown fields are ignored, so
    // their settings keep working (they drop the kept form on their next save).
    #[derive(serde::Deserialize)]
    struct OlderSettings {
        autolock: AutoLock,
        crisis_country: String,
    }
    let (_fx, mut j) = create();
    j.keep_unsaved_entry(unsaved_draft("synthetic"), NOW)
        .unwrap();
    let bytes = encode(j.settings()).unwrap();
    let older: OlderSettings = mossbook_core::domain::decode(&bytes).unwrap();
    assert_eq!(older.autolock, AutoLock::Immediately);
    assert_eq!(older.crisis_country, "PL");

    // And settings written before the field existed decode without one.
    #[derive(serde::Serialize)]
    struct Before {
        crisis_country: &'static str,
    }
    let before = encode(&Before {
        crisis_country: "PL",
    })
    .unwrap();
    let settings: Settings = mossbook_core::domain::decode(&before).unwrap();
    assert!(settings.unsaved_entry.is_none());
}

#[test]
fn json_export_leaves_out_the_unsaved_entry() {
    let (_fx, mut j) = create();
    j.keep_unsaved_entry(unsaved_draft("synthetic"), NOW)
        .unwrap();
    let bytes = j.export_json(NOW).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(doc["settings"].get("unsaved_entry").is_none());
    assert!(!String::from_utf8_lossy(&bytes).contains("synthetic trigger"));
    assert!(j.unsaved_entry().is_some(), "exporting does not discard it");
}

// ---- voice notes (E-18, D-21) ----

fn raw_vault_bytes(fx: &Fixture) -> Vec<u8> {
    let mut all = std::fs::read(&fx.path).unwrap();
    let mut wal = fx.path.clone().into_os_string();
    wal.push("-wal");
    if let Ok(w) = std::fs::read(wal) {
        all.extend(w);
    }
    all
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn note_with_voice(j: &mut Journal, packets: usize, seed: u8) -> Entry {
    let mut voice = j.save_voice(&common::ogg_opus(packets, seed)).unwrap();
    voice.duration_ms = 999_999; // claimed by the UI: must be ignored
    let mut e = Entry::new(EntryKind::Note, NOW).unwrap();
    e.voice = Some(voice);
    j.save_entry(e, NOW).unwrap().clone()
}

#[test]
fn a_recording_is_stored_encrypted_and_read_back() {
    let (fx, mut j) = create();
    let recording = common::ogg_opus(150, 7);
    let e = note_with_voice(&mut j, 150, 7);
    let voice = e.voice.clone().unwrap();
    assert_eq!(voice.duration_ms, 3000, "length taken from the file");
    assert_eq!(voice.mime, "audio/ogg; codecs=opus");
    assert_eq!(j.voice(&voice.id).unwrap().as_slice(), recording.as_slice());

    let j = reopen(&fx, j);
    assert_eq!(j.entry(&e.id).unwrap().voice, Some(voice.clone()));
    assert_eq!(j.voice(&voice.id).unwrap().as_slice(), recording.as_slice());
    assert_eq!(j.vault().unreadable_records(), 0);
    j.lock().unwrap();
    // The packet bytes are distinctive; none of them is on disk in the clear.
    assert!(!contains(&raw_vault_bytes(&fx), b"OpusHead"));
    assert!(!contains(&raw_vault_bytes(&fx), &[0xf8, 7, 3, 0x55, 0xaa]));
}

#[test]
fn a_transcript_is_kept_encrypted_and_searchable() {
    let (fx, mut j) = create();
    let mut e = note_with_voice(&mut j, 50, 4);
    // Without a transcript the entry encodes exactly as before (D-23): no new key.
    assert!(!contains(&encode(&e).unwrap(), b"transcript"));

    e.voice.as_mut().unwrap().transcript = "Synthetic walk with Żaneta".into();
    j.save_entry(e.clone(), NOW).unwrap();
    let j = reopen(&fx, j);
    let kept = j.entry(&e.id).unwrap().voice.clone().unwrap();
    assert_eq!(kept.transcript, "Synthetic walk with Żaneta");
    assert_eq!(
        kept.duration_ms, 1000,
        "the length still comes from the file"
    );
    let found = j.entries(&EntryFilter {
        text: Some("zaneta".into()),
        ..Default::default()
    });
    assert_eq!(found.len(), 1, "found by its transcript");
    assert_eq!(found[0].id, e.id);
    // The readable copy carries it (export version 3).
    let doc: serde_json::Value = serde_json::from_slice(&j.export_json(NOW).unwrap()).unwrap();
    assert_eq!(doc["version"], 4);
    assert_eq!(
        doc["entries"][0]["voice"]["transcript"],
        "Synthetic walk with Żaneta"
    );
    j.lock().unwrap();
    assert!(!contains(&raw_vault_bytes(&fx), b"Synthetic walk"));
}

#[test]
fn only_valid_recordings_are_accepted() {
    let (_fx, mut j) = create();
    let invalid = Err(JournalError::Domain(DomainError::InvalidField("voice")));
    assert_eq!(j.save_voice(b"\x1aE\xdf\xa3 webm").map(drop), invalid);
    assert_eq!(j.save_voice(&[]).map(drop), invalid);
    // Over five minutes.
    assert_eq!(
        j.save_voice(&common::ogg_opus(15_100, 1)).map(drop),
        invalid
    );
    assert!(j.vault().media_ids().unwrap().is_empty(), "nothing stored");

    // An entry cannot point to a recording that does not exist...
    let mut e = Entry::new(EntryKind::Note, NOW).unwrap();
    let mut ghost = j.save_voice(&common::ogg_opus(5, 1)).unwrap();
    ghost.id = Id::new_random().unwrap();
    e.voice = Some(ghost);
    assert_eq!(j.save_entry(e, NOW).map(drop), invalid);
    // ...or to another entry's recording.
    let first = note_with_voice(&mut j, 5, 2);
    let mut second = Entry::new(EntryKind::Note, NOW).unwrap();
    second.voice = first.voice.clone();
    assert_eq!(j.save_entry(second, NOW).map(drop), invalid);
}

#[test]
fn deleting_or_replacing_removes_the_old_recording() {
    let (_fx, mut j) = create();
    let e = note_with_voice(&mut j, 10, 1);
    let first = e.voice.clone().unwrap().id;

    // Replaced in the form: the old recording goes with the same save.
    let mut edited = e.clone();
    edited.voice = Some(j.save_voice(&common::ogg_opus(20, 2)).unwrap());
    let second = edited.voice.clone().unwrap().id;
    j.save_entry(edited.clone(), NOW).unwrap();
    assert_eq!(j.vault().media_ids().unwrap(), vec![*second.as_bytes()]);
    assert!(j.voice(&first).is_err());

    // Removed in the form.
    edited.voice = None;
    j.save_entry(edited.clone(), NOW).unwrap();
    assert!(j.vault().media_ids().unwrap().is_empty());

    // Deleting the entry deletes its recording.
    let e = note_with_voice(&mut j, 10, 3);
    assert_eq!(j.vault().media_ids().unwrap().len(), 1);
    assert!(j.delete_entry(&e.id, NOW).unwrap());
    assert!(j.vault().media_ids().unwrap().is_empty());
}

#[test]
fn recordings_nothing_points_to_are_removed_on_lock() {
    let (fx, mut j) = create();
    let kept = note_with_voice(&mut j, 10, 1).voice.clone().unwrap();
    // Recorded in a form that was left without saving.
    let orphan = j.save_voice(&common::ogg_opus(10, 2)).unwrap();
    // Recorded in a form that was kept for the lock (E-17, D-20).
    let in_form = j.save_voice(&common::ogg_opus(10, 3)).unwrap();
    let mut draft = unsaved_draft("with a recording");
    draft.voice = Some(in_form.clone());
    j.keep_unsaved_entry(draft, NOW).unwrap();

    let mut j = reopen(&fx, j);
    let mut ids = j.vault().media_ids().unwrap();
    ids.sort();
    let mut expected = vec![*kept.id.as_bytes(), *in_form.id.as_bytes()];
    expected.sort();
    assert_eq!(ids, expected);
    assert!(j.voice(&orphan.id).is_err());

    // The kept form is saved: its recording now belongs to the entry.
    let draft = j.unsaved_entry().unwrap().draft.clone();
    let saved = j.save_draft(&draft, NOW).unwrap().clone();
    assert_eq!(saved.voice.as_ref().map(|v| v.id), Some(in_form.id));
    let j = reopen(&fx, j);
    assert!(j.voice(&in_form.id).is_ok());
}

#[test]
fn discarding_the_kept_form_removes_its_recording() {
    let (_fx, mut j) = create();
    let in_form = j.save_voice(&common::ogg_opus(10, 3)).unwrap();
    let mut draft = unsaved_draft("with a recording");
    draft.voice = Some(in_form.clone());
    j.keep_unsaved_entry(draft, NOW).unwrap();
    j.discard_unsaved_entry().unwrap();
    assert!(j.vault().media_ids().unwrap().is_empty());
}

#[test]
fn entries_without_a_recording_encode_as_before() {
    let e = Entry::new(EntryKind::Note, NOW).unwrap();
    let bytes = encode(&e).unwrap();
    assert!(!contains(&bytes, b"voice"));
    assert!(!contains(&bytes, b"discuss"));
}

// ---- preparing for a session (E-7, E-20, D-26) ----

fn flagged_note(j: &mut Journal, text: &str) -> Id {
    let mut e = Entry::new(EntryKind::Note, NOW).unwrap();
    e.note = text.into();
    e.discuss = true;
    j.save_entry(e, NOW).unwrap().id
}

fn session(discussed: Vec<Id>) -> Entry {
    Entry::new(
        EntryKind::Session {
            agreed: "synthetic agreement".into(),
            todo: "synthetic homework".into(),
            discussed,
        },
        NOW,
    )
    .unwrap()
}

#[test]
fn saving_a_session_clears_the_flag_of_what_was_talked_about() {
    let (fx, mut j) = create();
    let a = flagged_note(&mut j, "synthetic topic A");
    let b = flagged_note(&mut j, "synthetic topic B");
    let later = NOW + Duration::hours(2);
    let id = j.save_entry(session(vec![a]), later).unwrap().id;
    assert!(!j.entry(&a).unwrap().discuss);
    assert_eq!(
        j.entry(&a).unwrap().updated_at,
        Timestamp(later),
        "travels by sync"
    );
    assert!(
        j.entry(&b).unwrap().discuss,
        "not talked about: waits for the next one"
    );

    let flagged = EntryFilter {
        discuss_only: true,
        ..EntryFilter::default()
    };
    let j = reopen(&fx, j);
    let still: Vec<Id> = j.entries(&flagged).iter().map(|e| e.id).collect();
    assert_eq!(still, vec![b]);
    // The text is searchable like the rest.
    let search = EntryFilter {
        text: Some("homework".into()),
        ..EntryFilter::default()
    };
    assert_eq!(j.entries(&search)[0].id, id);
}

#[test]
fn a_session_lists_only_existing_entries_once_and_not_itself() {
    let (_fx, mut j) = create();
    let a = flagged_note(&mut j, "synthetic topic A");
    let gone = flagged_note(&mut j, "synthetic deleted topic");
    j.delete_entry(&gone, NOW).unwrap();
    let mut s = session(Vec::new());
    let own = s.id;
    if let EntryKind::Session { discussed, .. } = &mut s.kind {
        *discussed = vec![a, gone, a, own, Id::from_bytes([7; 16])];
    }
    let saved = j.save_entry(s, NOW).unwrap();
    assert_eq!(
        saved.kind,
        EntryKind::Session {
            agreed: "synthetic agreement".into(),
            todo: "synthetic homework".into(),
            discussed: vec![a],
        }
    );
    // Flagged again for the next session, then the old session is edited:
    // the new flag stays.
    let mut again = j.entry(&a).unwrap().clone();
    again.discuss = true;
    j.save_entry(again, NOW).unwrap();
    let mut edited = j.entry(&own).unwrap().clone();
    edited.note = "synthetic edit".into();
    j.save_entry(edited, NOW).unwrap();
    assert!(j.entry(&a).unwrap().discuss);
}

#[test]
fn a_session_cannot_be_flagged_quick_or_list_too_much() {
    let (_fx, mut j) = create();
    let mut flagged = session(Vec::new());
    flagged.discuss = true;
    assert_eq!(
        j.save_entry(flagged, NOW).unwrap_err(),
        JournalError::Domain(DomainError::InvalidField("discuss"))
    );
    assert_eq!(
        j.quick_entry(EntryKindTag::Session, None, NOW).unwrap_err(),
        JournalError::Domain(DomainError::InvalidField("kind"))
    );
    let too_many = (0..=mossbook_core::domain::MAX_DISCUSSED)
        .map(|i| Id::from_bytes([(i % 251) as u8; 16]))
        .collect();
    assert_eq!(
        j.save_entry(session(too_many), NOW).unwrap_err(),
        JournalError::Domain(DomainError::InvalidField("discussed"))
    );
    assert!(j.entries(&EntryFilter::default()).is_empty());
}

#[test]
fn the_flag_and_sessions_reach_the_readable_copy() {
    let (_fx, mut j) = create();
    let a = flagged_note(&mut j, "synthetic topic");
    flagged_note(&mut j, "synthetic open question");
    j.save_entry(session(vec![a]), NOW).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&j.export_json(NOW).unwrap()).unwrap();
    assert_eq!(doc["version"], 4);
    let entries = doc["entries"].as_array().unwrap();
    let open = entries
        .iter()
        .find(|e| e["note"] == "synthetic open question")
        .unwrap();
    assert_eq!(open["discuss"], true);
    let s = entries
        .iter()
        .find(|e| e["kind"].get("session").is_some())
        .unwrap();
    assert_eq!(s["kind"]["session"]["todo"], "synthetic homework");
    assert_eq!(s["kind"]["session"]["discussed"][0], a.to_string());
}

#[test]
fn readable_copy_is_a_zip_with_the_json_and_the_recordings() {
    use std::io::Read;
    let (_fx, mut j) = create();
    let spoken = note_with_voice(&mut j, 50, 9);
    let mut plain = Entry::new(EntryKind::Note, NOW).unwrap();
    plain.note = "no recording".into();
    j.save_entry(plain, NOW).unwrap();

    let bytes = j.export_zip(NOW).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice())).unwrap();
    let voice = spoken.voice.clone().unwrap();
    let voice_name = format!("voice/{}.ogg", voice.id);
    let mut names: Vec<String> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["mossbook-export.json".to_owned(), voice_name.clone()]
    );

    let mut json = String::new();
    zip.by_name("mossbook-export.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["version"], 4);
    let entries = json["entries"].as_array().unwrap();
    let with_voice: Vec<_> = entries
        .iter()
        .filter(|e| e.get("voice").is_some())
        .collect();
    assert_eq!(with_voice.len(), 1);
    assert_eq!(with_voice[0]["voice"]["id"], voice.id.to_string());
    assert_eq!(with_voice[0]["voice"]["duration_ms"], 1000);
    assert!(
        with_voice[0]["voice"].get("transcript").is_none(),
        "left out when there is none"
    );

    let mut file = zip.by_name(&voice_name).unwrap();
    assert_eq!(file.compression(), zip::CompressionMethod::Stored);
    // No timestamp: the archive does not tell when it was made.
    assert_eq!(file.last_modified(), Some(zip::DateTime::default()));
    let mut audio = Vec::new();
    file.read_to_end(&mut audio).unwrap();
    assert_eq!(audio, common::ogg_opus(50, 9));
}

//! Integration tests for the journal on a real vault file. All data is synthetic.

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
    assert!(j.delete_entry(&nightmare).unwrap());

    assert!(j.delete_entry(&fear).unwrap());
    assert!(!j.delete_entry(&fear).unwrap());
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
    let work = j.create_tag("Praca", "#336699").unwrap().id;
    let job = j.create_tag("robota", "#336699").unwrap().id;
    let sleep = j.create_tag("sen", "#224466").unwrap().id;
    assert_eq!(
        j.create_tag(" praca ", "#000000").unwrap_err(),
        JournalError::DuplicateTagName
    );

    let mut renamed = j
        .tags()
        .into_iter()
        .find(|t| t.id == sleep)
        .unwrap()
        .clone();
    renamed.name = "Sen".into();
    j.save_tag(renamed).unwrap();

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
    j.save_plan(plan).unwrap();

    let mut settings = Settings::default();
    settings.autolock = AutoLock::After5Min;
    settings.locale = Some("uk".into());
    j.save_settings(settings.clone()).unwrap();
    let mut bad = settings.clone();
    bad.locale = Some("xx".into());
    assert!(j.save_settings(bad).is_err());

    let j = reopen(&fx, j);
    assert_eq!(j.plan().self_coping, vec!["synthetic walk".to_string()]);
    assert_eq!(j.plan().contacts[0].name, "Synthetic Friend");
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
    j.save_settings(settings).unwrap();
    let j = reopen(&fx, j);
    assert_eq!(j.settings().last_export, Some(Timestamp(NOW)));
    assert!(!j.settings().auto_backup);
}

#[test]
fn json_export_holds_the_whole_journal() {
    let (_fx, mut j) = create();
    let tag = j.create_tag("synthetic tag", "#2e7d4f").unwrap().id;
    let later = j.quick_entry(EntryKindTag::Fear, Some(7), NOW).unwrap().id;
    let earlier = j
        .quick_entry(EntryKindTag::Note, None, NOW - Duration::days(1))
        .unwrap()
        .id;
    let mut plan = SafetyPlan::default();
    plan.warning_signs = vec!["synthetic sign".into()];
    j.save_plan(plan).unwrap();

    let bytes = j.export_json(NOW).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["format"], "mossbook-export");
    assert_eq!(doc["version"], 1);
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
    )
        .prop_map(
            |(kind, occurred_at, mood, intensity, note, incomplete, sensitive, id)| {
                let mut e = Entry::new(kind, NOW).unwrap();
                e.id = Id::from_bytes(id);
                e.occurred_at = occurred_at;
                e.mood = mood;
                e.intensity = intensity;
                e.note = note;
                e.incomplete = incomplete;
                e.sensitive = sensitive;
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

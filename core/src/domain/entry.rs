//! Journal entries (SPEC §4.2, §5).

use core::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{DomainError, Id, Timestamp, VoiceNote, check_range};

/// Current schema version of [`Entry`] records.
pub const ENTRY_SCHEMA: u16 = 1;

/// A journal entry. Only `kind` differs between entry types.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Entry {
    pub id: Id,
    pub schema: u16,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    /// When the described thing happened (defaults to "now", editable).
    pub occurred_at: Timestamp,
    pub kind: EntryKind,
    /// Well-being rating, 1..=5.
    #[serde(default)]
    pub mood: Option<u8>,
    /// Intensity, 0..=10.
    #[serde(default)]
    pub intensity: Option<u8>,
    #[serde(default)]
    pub tags: Vec<Id>,
    #[serde(default)]
    pub note: String,
    /// Quick entry still to be completed (E-10).
    #[serde(default)]
    pub incomplete: bool,
    /// Content hidden until tapped (E-16).
    #[serde(default)]
    pub sensitive: bool,
    /// A recording attached to the entry (E-18). Left out when absent, so
    /// entries without one encode exactly as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<VoiceNote>,
    /// Marked to bring to the next therapy session (E-20, D-26). Left out when
    /// false, so entries without it encode exactly as before.
    #[serde(default, skip_serializing_if = "is_false")]
    pub discuss: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

/// Most entries one session can list as talked about (E-7).
pub const MAX_DISCUSSED: usize = 1000;

impl Entry {
    /// A new, empty entry of the given kind, happening at `now`.
    pub fn new(kind: EntryKind, now: OffsetDateTime) -> Result<Self, DomainError> {
        Ok(Self {
            id: Id::new_random()?,
            schema: ENTRY_SCHEMA,
            created_at: now.into(),
            updated_at: now.into(),
            occurred_at: now.into(),
            kind,
            mood: None,
            intensity: None,
            tags: Vec::new(),
            note: String::new(),
            incomplete: false,
            sensitive: false,
            voice: None,
            discuss: false,
        })
    }

    /// Range checks for every numeric field.
    pub fn validate(&self) -> Result<(), DomainError> {
        // A session is what brings flagged entries up; it is not one itself.
        if self.discuss && matches!(self.kind, EntryKind::Session { .. }) {
            return Err(DomainError::InvalidField("discuss"));
        }
        check_range(self.mood, 1..=5, "mood")?;
        check_range(self.intensity, 0..=10, "intensity")?;
        self.kind.validate()
    }

    /// Every piece of free text in the entry, for in-memory search.
    pub fn texts(&self) -> impl Iterator<Item = &str> {
        core::iter::once(self.note.as_str())
            .chain(self.kind.texts())
            .chain(self.voice.iter().map(|v| v.transcript.as_str()))
    }
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("kind", &self.kind.tag())
            .field("incomplete", &self.incomplete)
            .finish_non_exhaustive()
    }
}

/// A quick entry written while the vault is locked (E-15). Deliberately minimal:
/// anyone holding the vault file can append to the inbox, so it can only ever
/// produce a "to complete" entry of a known type with an in-range value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxEntry {
    pub kind: EntryKindTag,
    /// Mood rating 1..=5 for `mood`, intensity 0..=10 otherwise.
    pub value: Option<u8>,
    pub occurred_at: Timestamp,
}

impl InboxEntry {
    /// The entry it becomes once the vault is unlocked.
    pub fn into_entry(self, id: Id, received: OffsetDateTime) -> Result<Entry, DomainError> {
        if !self.kind.quick() {
            return Err(DomainError::InvalidField("kind"));
        }
        let mut e = Entry::new(EntryKind::empty(self.kind), received)?;
        e.id = id;
        e.occurred_at = self.occurred_at;
        match self.kind {
            EntryKindTag::Mood => e.mood = self.value,
            _ => e.intensity = self.value,
        }
        e.incomplete = true;
        e.sensitive = self.kind == EntryKindTag::Nightmare;
        e.validate()?;
        Ok(e)
    }
}

/// An entry as submitted by the UI. The journal owns `id` (for new entries),
/// `schema`, `created_at` and `updated_at`; the UI cannot set them.
/// Serialized only as part of an unsaved entry (see `settings::UnsavedEntry`).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct EntryDraft {
    /// `None` creates a new entry; `Some` edits an existing one.
    #[serde(default)]
    pub id: Option<Id>,
    pub kind: EntryKind,
    /// Defaults to "now" for new entries and is kept for edits.
    #[serde(default)]
    pub occurred_at: Option<Timestamp>,
    #[serde(default)]
    pub mood: Option<u8>,
    #[serde(default)]
    pub intensity: Option<u8>,
    #[serde(default)]
    pub tags: Vec<Id>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub incomplete: bool,
    #[serde(default)]
    pub sensitive: bool,
    /// The recording returned by the journal when it was stored; only its id
    /// is trusted, the length is taken from the file again on save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<VoiceNote>,
    #[serde(default)]
    pub discuss: bool,
}

impl EntryDraft {
    /// Applies the draft onto `entry` (a new or an existing one).
    pub fn apply_to(&self, entry: &mut Entry) {
        entry.kind = self.kind.clone();
        if let Some(at) = self.occurred_at {
            entry.occurred_at = at;
        }
        entry.mood = self.mood;
        entry.intensity = self.intensity;
        entry.tags = self.tags.clone();
        entry.note = self.note.clone();
        entry.incomplete = self.incomplete;
        entry.sensitive = self.sensitive;
        entry.voice = self.voice.clone();
        entry.discuss = self.discuss;
    }
}

impl fmt::Debug for EntryDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryDraft")
            .field("id", &self.id)
            .field("kind", &self.kind.tag())
            .finish_non_exhaustive()
    }
}

/// Entry type with its specific fields.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// E-1: well-being. The 1..=5 rating is [`Entry::mood`].
    Mood {
        #[serde(default)]
        energy: Option<u8>,
        #[serde(default)]
        emotions: Vec<Emotion>,
    },
    /// E-2: something that happened. Place is free text, never GPS.
    Event {
        #[serde(default)]
        what: String,
        #[serde(default)]
        place: String,
        #[serde(default)]
        body: String,
        #[serde(default)]
        thoughts: String,
        #[serde(default)]
        helped: String,
    },
    /// E-3: fear or trigger. Intensity before is [`Entry::intensity`].
    Fear {
        #[serde(default)]
        trigger: String,
        #[serde(default)]
        symptoms: Vec<Symptom>,
        #[serde(default)]
        avoidance: Option<bool>,
        #[serde(default)]
        strategy: Option<String>,
        #[serde(default)]
        intensity_after: Option<u8>,
    },
    /// E-4: sleep.
    Sleep {
        #[serde(default)]
        bedtime: Option<Timestamp>,
        #[serde(default)]
        wake: Option<Timestamp>,
        #[serde(default)]
        quality: Option<u8>,
        #[serde(default)]
        awakenings: Option<u8>,
        #[serde(default)]
        nightmare: bool,
    },
    /// E-5 (v1): nightmare and its rescripted version (IRT).
    Nightmare {
        #[serde(default)]
        content: String,
        #[serde(default)]
        rescripted: String,
        #[serde(default)]
        rehearsals: Vec<Timestamp>,
    },
    /// E-6: free note; the text is [`Entry::note`].
    Note,
    /// E-7 (D-26): notes after a therapy session.
    Session {
        /// What was agreed.
        #[serde(default)]
        agreed: String,
        /// What to do before the next session.
        #[serde(default)]
        todo: String,
        /// Entries flagged for the session that were talked about; saving the
        /// session clears their flag.
        #[serde(default)]
        discussed: Vec<Id>,
    },
}

impl EntryKind {
    /// An empty value of the given type, e.g. for a quick entry.
    pub fn empty(tag: EntryKindTag) -> Self {
        match tag {
            EntryKindTag::Mood => Self::Mood {
                energy: None,
                emotions: Vec::new(),
            },
            EntryKindTag::Event => Self::Event {
                what: String::new(),
                place: String::new(),
                body: String::new(),
                thoughts: String::new(),
                helped: String::new(),
            },
            EntryKindTag::Fear => Self::Fear {
                trigger: String::new(),
                symptoms: Vec::new(),
                avoidance: None,
                strategy: None,
                intensity_after: None,
            },
            EntryKindTag::Sleep => Self::Sleep {
                bedtime: None,
                wake: None,
                quality: None,
                awakenings: None,
                nightmare: false,
            },
            EntryKindTag::Nightmare => Self::Nightmare {
                content: String::new(),
                rescripted: String::new(),
                rehearsals: Vec::new(),
            },
            EntryKindTag::Note => Self::Note,
            EntryKindTag::Session => Self::Session {
                agreed: String::new(),
                todo: String::new(),
                discussed: Vec::new(),
            },
        }
    }

    pub fn tag(&self) -> EntryKindTag {
        match self {
            Self::Mood { .. } => EntryKindTag::Mood,
            Self::Event { .. } => EntryKindTag::Event,
            Self::Fear { .. } => EntryKindTag::Fear,
            Self::Sleep { .. } => EntryKindTag::Sleep,
            Self::Nightmare { .. } => EntryKindTag::Nightmare,
            Self::Note => EntryKindTag::Note,
            Self::Session { .. } => EntryKindTag::Session,
        }
    }

    fn validate(&self) -> Result<(), DomainError> {
        match self {
            Self::Mood { energy, .. } => check_range(*energy, 1..=5, "energy"),
            Self::Fear {
                intensity_after, ..
            } => check_range(*intensity_after, 0..=10, "intensity_after"),
            Self::Sleep { quality, .. } => check_range(*quality, 1..=5, "quality"),
            Self::Session { discussed, .. } if discussed.len() > MAX_DISCUSSED => {
                Err(DomainError::InvalidField("discussed"))
            }
            Self::Event { .. } | Self::Nightmare { .. } | Self::Note | Self::Session { .. } => {
                Ok(())
            }
        }
    }

    fn texts(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self {
            Self::Mood { emotions, .. } => {
                Box::new(emotions.iter().filter_map(Emotion::custom_text))
            }
            Self::Event {
                what,
                place,
                body,
                thoughts,
                helped,
            } => Box::new(
                [what, place, body, thoughts, helped]
                    .into_iter()
                    .map(String::as_str),
            ),
            Self::Fear {
                trigger,
                symptoms,
                strategy,
                ..
            } => Box::new(
                core::iter::once(trigger.as_str())
                    .chain(strategy.as_deref())
                    .chain(symptoms.iter().filter_map(Symptom::custom_text)),
            ),
            Self::Nightmare {
                content,
                rescripted,
                ..
            } => Box::new([content.as_str(), rescripted.as_str()].into_iter()),
            Self::Session { agreed, todo, .. } => {
                Box::new([agreed.as_str(), todo.as_str()].into_iter())
            }
            Self::Sleep { .. } | Self::Note => Box::new(core::iter::empty()),
        }
    }
}

impl fmt::Debug for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}([REDACTED])", self.tag())
    }
}

/// Entry type without its data, for filters and the quick entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKindTag {
    Mood,
    Event,
    Fear,
    Sleep,
    Nightmare,
    Note,
    Session,
}

impl EntryKindTag {
    /// Whether quick entry offers this type: a type plus one value (E-10, E-15).
    /// A session is written in the form only.
    pub fn quick(self) -> bool {
        self != Self::Session
    }
}

/// Emotion; the snake_case names double as translation keys (`emotion.<name>`).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    Anxiety,
    Sadness,
    Anger,
    Shame,
    Guilt,
    Relief,
    Calm,
    Joy,
    Loneliness,
    Numbness,
    Overwhelm,
    Custom(String),
}

impl fmt::Debug for Emotion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(_) => f.write_str("Custom([REDACTED])"),
            other => fmt::Debug::fmt(&serde_name(other), f),
        }
    }
}

impl Emotion {
    fn custom_text(&self) -> Option<&str> {
        match self {
            Self::Custom(text) => Some(text),
            _ => None,
        }
    }
}

/// Physical symptom; names double as translation keys (`symptom.<name>`).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum Symptom {
    RacingHeart,
    ShortnessOfBreath,
    ChestTightness,
    Sweating,
    Trembling,
    Nausea,
    Dizziness,
    MuscleTension,
    HotOrColdFlushes,
    Tingling,
    Unreality,
    Custom(String),
}

impl fmt::Debug for Symptom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(_) => f.write_str("Custom([REDACTED])"),
            other => fmt::Debug::fmt(&serde_name(other), f),
        }
    }
}

impl Symptom {
    fn custom_text(&self) -> Option<&str> {
        match self {
            Self::Custom(text) => Some(text),
            _ => None,
        }
    }
}

/// The serde (snake_case) name of a unit variant, used by redacted `Debug` impls.
fn serde_name<T: Serialize>(value: &T) -> String {
    match ciborium::Value::serialized(value) {
        Ok(ciborium::Value::Text(name)) => name,
        _ => String::from("?"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{decode, encode};
    use time::macros::datetime;

    const NOW: OffsetDateTime = datetime!(2026-10-19 21:30 +02:00);

    fn full_fear() -> Entry {
        let mut e = Entry::new(
            EntryKind::Fear {
                trigger: "crowded tram".into(),
                symptoms: vec![Symptom::RacingHeart, Symptom::Custom("cold hands".into())],
                avoidance: Some(true),
                strategy: Some("box breathing".into()),
                intensity_after: Some(4),
            },
            NOW,
        )
        .unwrap();
        e.intensity = Some(8);
        e.mood = Some(2);
        e.note = "synthetic note".into();
        e
    }

    #[test]
    fn cbor_round_trip_every_kind() {
        for tag in [
            EntryKindTag::Mood,
            EntryKindTag::Event,
            EntryKindTag::Fear,
            EntryKindTag::Sleep,
            EntryKindTag::Nightmare,
            EntryKindTag::Note,
        ] {
            let e = Entry::new(EntryKind::empty(tag), NOW).unwrap();
            let back: Entry = decode(&encode(&e).unwrap()).unwrap();
            assert_eq!(back, e);
            assert_eq!(back.kind.tag(), tag);
        }
        let e = full_fear();
        assert_eq!(decode::<Entry>(&encode(&e).unwrap()).unwrap(), e);
    }

    #[test]
    fn offset_is_preserved() {
        let e = Entry::new(EntryKind::Note, NOW).unwrap();
        let back: Entry = decode(&encode(&e).unwrap()).unwrap();
        assert_eq!(back.occurred_at.0.offset(), NOW.offset());
    }

    #[test]
    fn missing_optional_fields_default() {
        // A minimal record, as an older or foreign writer might produce it.
        #[derive(Serialize)]
        struct Minimal {
            id: Id,
            schema: u16,
            created_at: Timestamp,
            updated_at: Timestamp,
            occurred_at: Timestamp,
            kind: &'static str,
        }
        let t = Timestamp(NOW);
        let bytes = encode(&Minimal {
            id: Id::default(),
            schema: 1,
            created_at: t,
            updated_at: t,
            occurred_at: t,
            kind: "note",
        })
        .unwrap();
        let e: Entry = decode(&bytes).unwrap();
        assert_eq!(e.kind, EntryKind::Note);
        assert!(e.tags.is_empty() && e.note.is_empty() && e.mood.is_none());
    }

    #[test]
    fn ranges_are_validated() {
        let mut e = full_fear();
        assert_eq!(e.validate(), Ok(()));
        e.mood = Some(0);
        assert_eq!(e.validate(), Err(DomainError::InvalidField("mood")));
        e.mood = Some(5);
        e.intensity = Some(11);
        assert_eq!(e.validate(), Err(DomainError::InvalidField("intensity")));
        e.intensity = Some(0);
        if let EntryKind::Fear {
            intensity_after, ..
        } = &mut e.kind
        {
            *intensity_after = Some(11);
        }
        assert_eq!(
            e.validate(),
            Err(DomainError::InvalidField("intensity_after"))
        );

        let mut sleep = Entry::new(EntryKind::empty(EntryKindTag::Sleep), NOW).unwrap();
        if let EntryKind::Sleep { quality, .. } = &mut sleep.kind {
            *quality = Some(6);
        }
        assert_eq!(sleep.validate(), Err(DomainError::InvalidField("quality")));
        let mut mood = Entry::new(EntryKind::empty(EntryKindTag::Mood), NOW).unwrap();
        if let EntryKind::Mood { energy, .. } = &mut mood.kind {
            *energy = Some(0);
        }
        assert_eq!(mood.validate(), Err(DomainError::InvalidField("energy")));
    }

    #[test]
    fn texts_cover_free_text_fields() {
        let entry = full_fear();
        let texts: Vec<&str> = entry.texts().collect();
        for expected in [
            "synthetic note",
            "crowded tram",
            "box breathing",
            "cold hands",
        ] {
            assert!(texts.contains(&expected), "{expected}");
        }
    }

    #[test]
    fn debug_shows_no_content() {
        let dbg = format!("{:?}", full_fear());
        for secret in ["synthetic", "tram", "breathing", "cold hands", "2026"] {
            assert!(!dbg.contains(secret), "{dbg}");
        }
        assert!(dbg.contains("Fear"));
    }

    #[test]
    fn ids_are_random_and_carry_no_time() {
        // Two entries created at the same instant get unrelated v4 ids: nothing
        // in the plain-text row key reveals when an entry was written (D-13).
        let a = Entry::new(EntryKind::Note, NOW).unwrap();
        let b = Entry::new(EntryKind::Note, NOW).unwrap();
        assert_eq!(a.id.0.get_version_num(), 4);
        assert_ne!(a.id, b.id);
        assert_ne!(
            a.id.as_bytes()[..6],
            b.id.as_bytes()[..6],
            "no shared timestamp prefix"
        );
    }
}

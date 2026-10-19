//! The unlocked journal: decoded domain objects on top of an [`UnlockedVault`].
//!
//! Every change is encrypted and written first and applied in memory only after
//! the write succeeded, so memory never shows data that is not on disk.

use core::fmt;
use std::collections::HashMap;

use time::OffsetDateTime;

use crate::crypto::aead::RecordType;
use crate::domain::entry::ENTRY_SCHEMA;
use crate::domain::{
    DomainError, Entry, EntryKind, EntryKindTag, Id, SafetyPlan, Settings, Tag, decode, encode,
};
use crate::query::stats::{MoodPoint, mood_by_day};
use crate::query::{EntryFilter, StatsRange};
use crate::vault::{UnlockedVault, VaultError};

/// Fixed record id of the safety plan (a singleton).
pub const PLAN_ID: Id = Id::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3]);
/// Fixed record id of the settings (a singleton).
pub const SETTINGS_ID: Id = Id::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalError {
    Domain(DomainError),
    Vault(VaultError),
    /// Another tag already has this name (case-insensitive).
    DuplicateTagName,
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(e) => fmt::Display::fmt(e, f),
            Self::Vault(e) => fmt::Display::fmt(e, f),
            Self::DuplicateTagName => f.write_str("a tag with this name already exists"),
        }
    }
}

impl std::error::Error for JournalError {}

impl From<DomainError> for JournalError {
    fn from(e: DomainError) -> Self {
        Self::Domain(e)
    }
}

impl From<VaultError> for JournalError {
    fn from(e: VaultError) -> Self {
        Self::Vault(e)
    }
}

pub struct Journal {
    vault: UnlockedVault,
    entries: HashMap<Id, Entry>,
    tags: HashMap<Id, Tag>,
    plan: SafetyPlan,
    settings: Settings,
}

impl fmt::Debug for Journal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Journal")
            .field("entries", &self.entries.len())
            .field("tags", &self.tags.len())
            .finish_non_exhaustive()
    }
}

impl Journal {
    /// Decodes the records decrypted on unlock. Records that do not decode are
    /// added to the vault's unreadable count instead of failing the unlock.
    pub fn open(mut vault: UnlockedVault) -> Self {
        let mut entries = HashMap::new();
        let mut tags = HashMap::new();
        let mut plan = SafetyPlan::default();
        let mut settings = Settings::default();
        let mut bad = 0;
        for record in vault.take_loaded() {
            let id = Id::from_bytes(record.id);
            let bytes = record.plaintext.as_slice();
            // A decoded object must carry the id of the row it came from.
            let ok = match record.record_type {
                RecordType::Entry => match decode::<Entry>(bytes) {
                    Ok(e) if e.id == id => entries.insert(id, e).is_none(),
                    _ => false,
                },
                RecordType::Tag => match decode::<Tag>(bytes) {
                    Ok(t) if t.id == id => tags.insert(id, t).is_none(),
                    _ => false,
                },
                RecordType::Plan if id == PLAN_ID => decode(bytes).map(|p| plan = p).is_ok(),
                RecordType::Settings if id == SETTINGS_ID => {
                    decode(bytes).map(|s| settings = s).is_ok()
                }
                RecordType::Plan | RecordType::Settings => false,
            };
            if !ok {
                bad += 1;
            }
        }
        vault.add_unreadable(bad);
        Self {
            vault,
            entries,
            tags,
            plan,
            settings,
        }
    }

    pub fn vault(&self) -> &UnlockedVault {
        &self.vault
    }

    pub fn vault_mut(&mut self) -> &mut UnlockedVault {
        &mut self.vault
    }

    /// Locks: zeroes the DEK and drops (and zeroes) all decoded content.
    pub fn lock(self) -> Result<(), VaultError> {
        self.vault.lock()
    }

    /// Gives up the decoded content and returns the bare vault.
    pub fn into_vault(self) -> UnlockedVault {
        self.vault
    }

    // ---- entries ----

    pub fn entry(&self, id: &Id) -> Option<&Entry> {
        self.entries.get(id)
    }

    /// Entries matching `filter`, newest first.
    pub fn entries(&self, filter: &EntryFilter) -> Vec<&Entry> {
        filter.apply(self.entries.values(), &self.tags)
    }

    /// Creates or updates an entry. Keeps `created_at` of an existing entry,
    /// sets `updated_at = now`, drops duplicate tags and rejects unknown ones.
    pub fn save_entry(
        &mut self,
        mut entry: Entry,
        now: OffsetDateTime,
    ) -> Result<&Entry, JournalError> {
        entry.validate()?;
        let mut seen = Vec::with_capacity(entry.tags.len());
        for tag in &entry.tags {
            if !self.tags.contains_key(tag) {
                return Err(DomainError::InvalidField("tags").into());
            }
            if !seen.contains(tag) {
                seen.push(*tag);
            }
        }
        entry.tags = seen;
        entry.schema = ENTRY_SCHEMA;
        if let Some(existing) = self.entries.get(&entry.id) {
            entry.created_at = existing.created_at;
        }
        entry.updated_at = now.into();
        self.vault
            .put(*entry.id.as_bytes(), RecordType::Entry, &encode(&entry)?)?;
        let id = entry.id;
        self.entries.insert(id, entry);
        Ok(&self.entries[&id])
    }

    /// Quick entry (E-10): type plus one value, marked as incomplete.
    /// For mood the value is the 1..=5 rating, otherwise the 0..=10 intensity.
    pub fn quick_entry(
        &mut self,
        kind: EntryKindTag,
        value: Option<u8>,
        now: OffsetDateTime,
    ) -> Result<&Entry, JournalError> {
        let mut entry = Entry::new(EntryKind::empty(kind), now)?;
        match kind {
            EntryKindTag::Mood => entry.mood = value,
            _ => entry.intensity = value,
        }
        entry.incomplete = true;
        self.save_entry(entry, now)
    }

    pub fn delete_entry(&mut self, id: &Id) -> Result<bool, JournalError> {
        if self.entries.remove(id).is_none() {
            return Ok(false);
        }
        self.vault.delete(id.as_bytes())?;
        Ok(true)
    }

    // ---- tags ----

    /// All tags sorted by name.
    pub fn tags(&self) -> Vec<&Tag> {
        let mut tags: Vec<&Tag> = self.tags.values().collect();
        tags.sort_by_cached_key(|t| t.name.to_lowercase());
        tags
    }

    pub fn create_tag(
        &mut self,
        name: &str,
        color: &str,
        now: OffsetDateTime,
    ) -> Result<&Tag, JournalError> {
        let tag = Tag {
            id: Id::new_v7(now)?,
            name: name.to_owned(),
            color: color.to_owned(),
        };
        self.save_tag(tag)
    }

    /// Renames or recolors a tag (or creates it if the id is new).
    pub fn save_tag(&mut self, mut tag: Tag) -> Result<&Tag, JournalError> {
        tag.normalize()?;
        if self
            .tags
            .values()
            .any(|t| t.id != tag.id && t.same_name(&tag.name))
        {
            return Err(JournalError::DuplicateTagName);
        }
        self.vault
            .put(*tag.id.as_bytes(), RecordType::Tag, &encode(&tag)?)?;
        let id = tag.id;
        self.tags.insert(id, tag);
        Ok(&self.tags[&id])
    }

    /// Deletes a tag and removes it from every entry, in one transaction.
    pub fn delete_tag(&mut self, id: &Id, now: OffsetDateTime) -> Result<bool, JournalError> {
        if !self.tags.contains_key(id) {
            return Ok(false);
        }
        self.retag(id, None, now)?;
        Ok(true)
    }

    /// Merges `from` into `into`: entries tagged `from` get `into`, then `from`
    /// is deleted. One transaction.
    pub fn merge_tags(
        &mut self,
        from: &Id,
        into: &Id,
        now: OffsetDateTime,
    ) -> Result<(), JournalError> {
        if from == into || !self.tags.contains_key(from) || !self.tags.contains_key(into) {
            return Err(DomainError::NotFound.into());
        }
        self.retag(from, Some(*into), now)
    }

    /// Replaces tag `from` with `into` (or removes it) on all entries and deletes `from`.
    fn retag(
        &mut self,
        from: &Id,
        into: Option<Id>,
        now: OffsetDateTime,
    ) -> Result<(), JournalError> {
        let mut changed: Vec<Entry> = Vec::new();
        for entry in self.entries.values().filter(|e| e.tags.contains(from)) {
            let mut e = entry.clone();
            e.tags.retain(|t| t != from);
            if let Some(into) = into
                && !e.tags.contains(&into)
            {
                e.tags.push(into);
            }
            e.updated_at = now.into();
            changed.push(e);
        }
        let encoded = changed.iter().map(encode).collect::<Result<Vec<_>, _>>()?;
        let puts: Vec<_> = changed
            .iter()
            .zip(&encoded)
            .map(|(e, bytes)| (*e.id.as_bytes(), RecordType::Entry, bytes.as_slice()))
            .collect();
        self.vault.write_batch(&puts, &[*from.as_bytes()])?;
        for e in changed {
            self.entries.insert(e.id, e);
        }
        self.tags.remove(from);
        Ok(())
    }

    // ---- plan and settings ----

    pub fn plan(&self) -> &SafetyPlan {
        &self.plan
    }

    pub fn save_plan(&mut self, mut plan: SafetyPlan) -> Result<&SafetyPlan, JournalError> {
        plan.normalize();
        self.vault
            .put(*PLAN_ID.as_bytes(), RecordType::Plan, &encode(&plan)?)?;
        self.plan = plan;
        Ok(&self.plan)
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn save_settings(&mut self, settings: Settings) -> Result<&Settings, JournalError> {
        settings.validate()?;
        self.vault.put(
            *SETTINGS_ID.as_bytes(),
            RecordType::Settings,
            &encode(&settings)?,
        )?;
        self.settings = settings;
        Ok(&self.settings)
    }

    // ---- patterns ----

    /// Daily mood averages for the chart (A-1).
    pub fn mood_series(&self, range: StatsRange, now: OffsetDateTime) -> Vec<MoodPoint> {
        let (from, to) = range.bounds(now);
        mood_by_day(self.entries.values(), from, to)
    }
}

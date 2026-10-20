//! The unlocked journal: decoded domain objects on top of an [`UnlockedVault`].
//!
//! Every change is encrypted and written first and applied in memory only after
//! the write succeeded, so memory never shows data that is not on disk.

use core::fmt;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use zeroize::Zeroizing;

use crate::crypto::aead::RecordType;
use crate::domain::entry::ENTRY_SCHEMA;
use crate::domain::{
    DomainError, Entry, EntryDraft, EntryKind, EntryKindTag, Id, InboxEntry, SafetyPlan, Settings,
    Tag, decode, encode,
};
use crate::query::stats::{MoodPoint, mood_by_day};
use crate::query::{EntryFilter, Patterns, StatsRange};
use crate::vault::{LoadedRecord, UnlockedVault, VaultError};

/// `format` field of the plain JSON export.
pub const EXPORT_FORMAT: &str = "mossbook-export";
/// Version of the plain JSON export layout.
pub const EXPORT_VERSION: u16 = 1;

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

/// How a backup is brought into an existing journal (X-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Add what is missing; an entry in both keeps the newer `updated_at`;
    /// a tag with the same name is merged; the current plan and settings stay.
    Merge,
    /// The journal becomes exactly the backup's content.
    Replace,
}

/// What an import changed, for the confirmation shown to the user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// Records in the backup that could not be decoded or were invalid.
    pub skipped: usize,
}

/// Domain objects decoded from a list of records.
#[derive(Default)]
struct Decoded {
    entries: HashMap<Id, Entry>,
    tags: HashMap<Id, Tag>,
    plan: Option<SafetyPlan>,
    settings: Option<Settings>,
    bad: usize,
}

impl Decoded {
    /// Decodes records; one must carry the id of the row it came from.
    fn from_records(records: Vec<LoadedRecord>) -> Self {
        let mut d = Self::default();
        for record in records {
            let id = Id::from_bytes(record.id);
            let bytes = record.plaintext.as_slice();
            let ok = match record.record_type {
                RecordType::Entry => match decode::<Entry>(bytes) {
                    Ok(e) if e.id == id => d.entries.insert(id, e).is_none(),
                    _ => false,
                },
                RecordType::Tag => match decode::<Tag>(bytes) {
                    Ok(t) if t.id == id => d.tags.insert(id, t).is_none(),
                    _ => false,
                },
                RecordType::Plan if id == PLAN_ID => {
                    decode(bytes).map(|p| d.plan = Some(p)).is_ok()
                }
                RecordType::Settings if id == SETTINGS_ID => {
                    decode(bytes).map(|s| d.settings = Some(s)).is_ok()
                }
                RecordType::Plan | RecordType::Settings => false,
            };
            if !ok {
                d.bad += 1;
            }
        }
        d
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
        let d = Decoded::from_records(vault.take_loaded());
        vault.add_unreadable(d.bad);
        let mut journal = Self {
            vault,
            entries: d.entries,
            tags: d.tags,
            plan: d.plan.unwrap_or_default(),
            settings: d.settings.unwrap_or_default(),
        };
        journal.move_inbox_in(OffsetDateTime::now_utc());
        journal
    }

    /// Moves quick entries written while locked into the journal (E-15). They
    /// keep their inbox id, so a crash between storing and clearing only
    /// re-stores the same entries next time.
    fn move_inbox_in(&mut self, now: OffsetDateTime) {
        let items = self.vault.take_inbox();
        if items.is_empty() {
            return;
        }
        let mut entries = Vec::new();
        let mut bad = 0;
        for item in &items {
            match decode::<InboxEntry>(&item.plaintext)
                .and_then(|i| i.into_entry(Id::from_bytes(item.id), now))
            {
                Ok(e) => entries.push(e),
                Err(_) => bad += 1,
            }
        }
        let encoded: Result<Vec<_>, _> = entries.iter().map(encode).collect();
        let Ok(encoded) = encoded else { return };
        let puts: Vec<_> = entries
            .iter()
            .zip(&encoded)
            .map(|(e, bytes)| (*e.id.as_bytes(), RecordType::Entry, bytes.as_slice()))
            .collect();
        if self.vault.write_batch(&puts, &[]).is_err() {
            return; // left in the inbox for the next unlock
        }
        // Invalid items are dropped from the inbox too, after being counted.
        let ids: Vec<_> = items.iter().map(|i| i.id).collect();
        let _ = self.vault.clear_inbox(&ids);
        self.vault.add_unreadable(bad);
        for e in entries {
            self.entries.insert(e.id, e);
        }
    }

    /// Quick entry while the vault is locked (E-15): sealed into the inbox with
    /// the vault's public key. It shows up in the journal after the next unlock.
    pub fn quick_entry_locked(
        path: &std::path::Path,
        kind: EntryKindTag,
        value: Option<u8>,
        now: OffsetDateTime,
    ) -> Result<(), JournalError> {
        let item = InboxEntry {
            kind,
            value,
            occurred_at: now.into(),
        };
        // Same validation as after unlock, so a bad value fails now, not later.
        item.into_entry(Id::default(), now)?;
        crate::vault::append_to_inbox(path, &encode(&item)?)?;
        Ok(())
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

    /// Saves an entry submitted by the UI: creates it if `draft.id` is `None`,
    /// otherwise edits the existing entry (unknown ids are rejected).
    pub fn save_draft(
        &mut self,
        draft: &EntryDraft,
        now: OffsetDateTime,
    ) -> Result<&Entry, JournalError> {
        let mut entry = match draft.id {
            Some(id) => self
                .entries
                .get(&id)
                .cloned()
                .ok_or(DomainError::NotFound)?,
            None => Entry::new(draft.kind.clone(), now)?,
        };
        draft.apply_to(&mut entry);
        self.save_entry(entry, now)
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
        // A nightmare's content is hidden on the timeline by default (E-5, E-16).
        entry.sensitive = kind == EntryKindTag::Nightmare;
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

    pub fn create_tag(&mut self, name: &str, color: &str) -> Result<&Tag, JournalError> {
        let tag = Tag {
            id: Id::new_random()?,
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

    /// Saves the settings. `last_export` is kept from the stored settings: it is
    /// set only by [`Self::record_export`].
    pub fn save_settings(&mut self, mut settings: Settings) -> Result<&Settings, JournalError> {
        settings.validate()?;
        settings.last_export = self.settings.last_export;
        self.store_settings(settings)
    }

    /// Notes that the user saved a backup themselves (shown in the settings).
    pub fn record_export(&mut self, now: OffsetDateTime) -> Result<(), JournalError> {
        let mut settings = self.settings.clone();
        settings.last_export = Some(now.into());
        self.store_settings(settings).map(drop)
    }

    fn store_settings(&mut self, settings: Settings) -> Result<&Settings, JournalError> {
        self.vault.put(
            *SETTINGS_ID.as_bytes(),
            RecordType::Settings,
            &encode(&settings)?,
        )?;
        self.settings = settings;
        Ok(&self.settings)
    }

    // ---- plain export ----

    /// The whole journal as readable JSON (X-4): entries (oldest first), tags,
    /// the plan and the settings, in the same shapes the app uses internally.
    /// The result is **not encrypted**; the UI warns before saving it.
    /// Described in `docs/backup-format.md` (*Plain JSON export*).
    pub fn export_json(&self, now: OffsetDateTime) -> Result<Zeroizing<Vec<u8>>, JournalError> {
        #[derive(Serialize)]
        struct Export<'a> {
            format: &'static str,
            version: u16,
            exported_at: crate::domain::Timestamp,
            entries: Vec<&'a Entry>,
            tags: Vec<&'a Tag>,
            plan: &'a SafetyPlan,
            settings: &'a Settings,
        }
        let mut entries: Vec<&Entry> = self.entries.values().collect();
        entries.sort_by_key(|e| (e.occurred_at, e.id));
        let mut tags: Vec<&Tag> = self.tags.values().collect();
        tags.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        let export = Export {
            format: EXPORT_FORMAT,
            version: EXPORT_VERSION,
            exported_at: now.into(),
            entries,
            tags,
            plan: &self.plan,
            settings: &self.settings,
        };
        serde_json::to_vec_pretty(&export)
            .map(Zeroizing::new)
            .map_err(|_| DomainError::Encoding.into())
    }

    // ---- import ----

    /// Brings decrypted backup records into this journal, re-encrypted with
    /// this vault's key, in one transaction (X-2, decision D-4).
    pub fn import(
        &mut self,
        records: Vec<LoadedRecord>,
        mode: ImportMode,
    ) -> Result<ImportReport, JournalError> {
        let backup = Decoded::from_records(records);
        let mut report = ImportReport {
            skipped: backup.bad,
            ..ImportReport::default()
        };
        match mode {
            ImportMode::Replace => self.import_replace(backup, &mut report)?,
            ImportMode::Merge => self.import_merge(backup, &mut report)?,
        }
        Ok(report)
    }

    fn import_replace(
        &mut self,
        b: Decoded,
        report: &mut ImportReport,
    ) -> Result<(), JournalError> {
        let mut tags = HashMap::new();
        for (id, mut tag) in b.tags {
            match tag.normalize() {
                Ok(()) => drop(tags.insert(id, tag)),
                Err(_) => report.skipped += 1,
            }
        }
        let mut entries = HashMap::new();
        for (id, mut entry) in b.entries {
            entry.tags.retain(|t| tags.contains_key(t));
            match entry.validate() {
                Ok(()) => drop(entries.insert(id, entry)),
                Err(_) => report.skipped += 1,
            }
        }
        let plan = b.plan.unwrap_or_default();
        let settings = b
            .settings
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default();

        let mut encoded = Vec::new();
        for e in entries.values() {
            encoded.push((*e.id.as_bytes(), RecordType::Entry, encode(e)?));
        }
        for t in tags.values() {
            encoded.push((*t.id.as_bytes(), RecordType::Tag, encode(t)?));
        }
        encoded.push((*PLAN_ID.as_bytes(), RecordType::Plan, encode(&plan)?));
        encoded.push((
            *SETTINGS_ID.as_bytes(),
            RecordType::Settings,
            encode(&settings)?,
        ));
        let puts: Vec<_> = encoded
            .iter()
            .map(|(id, rt, bytes)| (*id, *rt, bytes.as_slice()))
            .collect();
        let deletes: Vec<_> = self
            .entries
            .keys()
            .chain(self.tags.keys())
            .filter(|id| !entries.contains_key(id) && !tags.contains_key(id))
            .map(|id| *id.as_bytes())
            .collect();
        self.vault.write_batch(&puts, &deletes)?;

        report.added = entries.len() + tags.len();
        self.entries = entries;
        self.tags = tags;
        self.plan = plan;
        self.settings = settings;
        Ok(())
    }

    fn import_merge(&mut self, b: Decoded, report: &mut ImportReport) -> Result<(), JournalError> {
        // Tags: same id → keep ours; same name → map onto ours; otherwise add.
        let mut tag_map: HashMap<Id, Id> = HashMap::new();
        let mut new_tags: Vec<Tag> = Vec::new();
        for (id, mut tag) in b.tags {
            if self.tags.contains_key(&id) {
                tag_map.insert(id, id);
                report.unchanged += 1;
            } else if let Some(ours) = self
                .tags
                .values()
                .chain(&new_tags)
                .find(|t| t.same_name(&tag.name))
            {
                tag_map.insert(id, ours.id);
                report.unchanged += 1;
            } else if tag.normalize().is_ok() {
                tag_map.insert(id, id);
                new_tags.push(tag);
            } else {
                report.skipped += 1;
            }
        }

        // Entries: add missing ones; for entries in both, the newer edit wins.
        let mut changed: Vec<Entry> = Vec::new();
        for (id, mut entry) in b.entries {
            let mut seen = Vec::new();
            for t in entry.tags.iter().filter_map(|t| tag_map.get(t)) {
                if !seen.contains(t) {
                    seen.push(*t);
                }
            }
            entry.tags = seen;
            if entry.validate().is_err() {
                report.skipped += 1;
                continue;
            }
            match self.entries.get(&id) {
                None => {
                    report.added += 1;
                    changed.push(entry);
                }
                Some(ours) if entry.updated_at > ours.updated_at => {
                    report.updated += 1;
                    changed.push(entry);
                }
                Some(_) => report.unchanged += 1,
            }
        }
        report.added += new_tags.len();

        let mut encoded = Vec::new();
        for t in &new_tags {
            encoded.push((*t.id.as_bytes(), RecordType::Tag, encode(t)?));
        }
        for e in &changed {
            encoded.push((*e.id.as_bytes(), RecordType::Entry, encode(e)?));
        }
        let puts: Vec<_> = encoded
            .iter()
            .map(|(id, rt, bytes)| (*id, *rt, bytes.as_slice()))
            .collect();
        self.vault.write_batch(&puts, &[])?;
        for t in new_tags {
            self.tags.insert(t.id, t);
        }
        for e in changed {
            self.entries.insert(e.id, e);
        }
        Ok(())
    }

    // ---- patterns ----

    /// Tags, triggers, mood by sleep, time of day and weekday, strategies (A-2–A-4).
    pub fn patterns(&self, range: StatsRange, now: OffsetDateTime) -> Patterns {
        let (from, to) = range.bounds(now);
        crate::query::patterns::patterns(self.entries.values(), &self.tags, from, to)
    }

    /// Daily mood averages for the chart (A-1).
    pub fn mood_series(&self, range: StatsRange, now: OffsetDateTime) -> Vec<MoodPoint> {
        let (from, to) = range.bounds(now);
        mood_by_day(self.entries.values(), from, to)
    }
}

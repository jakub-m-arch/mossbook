//! Merging another device's copy of the journal (sync, decision D-22).
//!
//! The user's devices hold the same journal (same vault id and key). Another
//! device's sync file is opened with that key and merged here record by record:
//! the newer change wins, deletions travel as tombstones, and every device ends
//! up with the same content whatever order the files arrive in.

use std::cmp::Ordering;
use std::collections::HashSet;

use serde::Serialize;
use time::OffsetDateTime;
use zeroize::Zeroizing;

use super::{Decoded, Journal, JournalError, PLAN_ID, SETTINGS_ID};
use crate::crypto::aead::RecordType;
use crate::domain::{Entry, Id, Tag, Timestamp, Tombstone, TombstoneOf, encode, inspect_ogg_opus};
use crate::vault::LoadedRecord;

/// What a sync changed on this device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    /// Entries and tags added or changed here (plan and settings included).
    pub changed: usize,
    /// Entries and tags deleted here.
    pub deleted: usize,
    /// Records in the other device's file that could not be used.
    pub skipped: usize,
}

/// One side of a comparison: a record with its encoding (for ties) or a
/// tombstone, and when it last changed. `None` is a record from before sync.
enum Version {
    Live(Option<Timestamp>, Zeroizing<Vec<u8>>),
    Dead(Timestamp),
}

impl Version {
    fn time(&self) -> Option<Timestamp> {
        match self {
            Self::Live(t, _) => *t,
            Self::Dead(t) => Some(*t),
        }
    }
}

/// Whether `remote` replaces `local`. The newer change wins; at the same time a
/// deletion beats an edit, and of two different edits the larger encoding
/// wins, so every device decides the same way.
fn remote_wins(local: Option<&Version>, remote: &Version) -> bool {
    let Some(local) = local else {
        return true;
    };
    match remote.time().cmp(&local.time()) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => match (local, remote) {
            (Version::Live(_, ours), Version::Live(_, theirs)) => {
                theirs.as_slice() > ours.as_slice()
            }
            (Version::Live(..), Version::Dead(_)) => true,
            (Version::Dead(_), _) => false,
        },
    }
}

impl Journal {
    fn tag_version(&self, id: &Id) -> Result<Option<Version>, JournalError> {
        if let Some(t) = self.tags.get(id) {
            return Ok(Some(Version::Live(t.updated_at, encode(t)?)));
        }
        Ok(self.dead(id))
    }

    fn entry_version(&self, id: &Id) -> Result<Option<Version>, JournalError> {
        if let Some(e) = self.entries.get(id) {
            return Ok(Some(Version::Live(Some(e.updated_at), encode(e)?)));
        }
        Ok(self.dead(id))
    }

    fn dead(&self, id: &Id) -> Option<Version> {
        self.tombstones.get(id).map(|t| Version::Dead(t.deleted_at))
    }

    /// Merges the records of another device's sync file into this journal.
    /// `voice` gives that file's recording for an id; a recording is copied
    /// only for an entry that wins and only if it is not here yet. Changes are
    /// written in one transaction (recordings just before it), then two tags
    /// with the same name become one and unused recordings are removed.
    pub fn merge_from_device(
        &mut self,
        records: Vec<LoadedRecord>,
        voice: &dyn Fn(&Id) -> Option<Zeroizing<Vec<u8>>>,
        now: OffsetDateTime,
    ) -> Result<SyncReport, JournalError> {
        let remote = Decoded::from_records(records);
        let mut report = SyncReport {
            skipped: remote.bad,
            ..SyncReport::default()
        };
        let mut puts: Vec<(Id, RecordType, Zeroizing<Vec<u8>>)> = Vec::new();

        // Tags first: entries refer to them.
        let mut tags_in: Vec<Tag> = Vec::new();
        for (id, mut tag) in remote.tags {
            if tag.normalize().is_err() {
                report.skipped += 1;
                continue;
            }
            let bytes = encode(&tag)?;
            let theirs = Version::Live(tag.updated_at, bytes);
            if remote_wins(self.tag_version(&id)?.as_ref(), &theirs) {
                let Version::Live(_, bytes) = theirs else {
                    unreachable!()
                };
                puts.push((id, RecordType::Tag, bytes));
                tags_in.push(tag);
            }
        }
        let mut deaths: Vec<Tombstone> = Vec::new();
        for (id, tombstone) in &remote.tombstones {
            let local = match tombstone.of {
                TombstoneOf::Tag => self.tag_version(id)?,
                TombstoneOf::Entry => self.entry_version(id)?,
            };
            if remote_wins(local.as_ref(), &Version::Dead(tombstone.deleted_at)) {
                puts.push((*id, RecordType::Tombstone, encode(tombstone)?));
                deaths.push(*tombstone);
            }
        }
        let dead_ids: HashSet<Id> = deaths.iter().map(|t| t.id).collect();
        let live_tags: HashSet<Id> = self
            .tags
            .keys()
            .copied()
            .chain(tags_in.iter().map(|t| t.id))
            .filter(|id| !dead_ids.contains(id))
            .collect();

        let mut entries_in: Vec<Entry> = Vec::new();
        for (id, mut entry) in remote.entries {
            if entry.validate().is_err() {
                report.skipped += 1;
                continue;
            }
            let theirs = Version::Live(Some(entry.updated_at), encode(&entry)?);
            if !remote_wins(self.entry_version(&id)?.as_ref(), &theirs) {
                continue;
            }
            entry.tags.retain(|t| live_tags.contains(t));
            self.receive_voice(&mut entry, voice)?;
            puts.push((id, RecordType::Entry, encode(&entry)?));
            entries_in.push(entry);
        }

        let mut plan_in = None;
        if let Some(mut plan) = remote.plan {
            plan.normalize();
            let ours = Version::Live(self.plan.updated_at, encode(&self.plan)?);
            let theirs = Version::Live(plan.updated_at, encode(&plan)?);
            if remote_wins(Some(&ours), &theirs) {
                puts.push((PLAN_ID, RecordType::Plan, encode(&plan)?));
                plan_in = Some(plan);
            }
        }
        let mut settings_in = None;
        if let Some(mut settings) = remote.settings.filter(|s| s.validate().is_ok()) {
            // The form open on this device stays this device's (D-20).
            let mut ours = self.settings.clone();
            ours.unsaved_entry = None;
            settings.unsaved_entry = None;
            let ours = Version::Live(ours.updated_at, encode(&ours)?);
            let theirs = Version::Live(settings.updated_at, encode(&settings)?);
            if remote_wins(Some(&ours), &theirs) {
                settings.unsaved_entry = self.settings.unsaved_entry.clone();
                puts.push((SETTINGS_ID, RecordType::Settings, encode(&settings)?));
                settings_in = Some(settings);
            }
        }

        let refs: Vec<_> = puts
            .iter()
            .map(|(id, rt, bytes)| (*id.as_bytes(), *rt, bytes.as_slice()))
            .collect();
        self.vault.write_batch(&refs, &[])?;

        report.changed = tags_in.len()
            + entries_in.len()
            + usize::from(plan_in.is_some())
            + usize::from(settings_in.is_some());
        for tag in tags_in {
            self.tombstones.remove(&tag.id);
            self.tags.insert(tag.id, tag);
        }
        for entry in entries_in {
            self.tombstones.remove(&entry.id);
            self.entries.insert(entry.id, entry);
        }
        for t in deaths {
            let was_live = match t.of {
                TombstoneOf::Tag => self.tags.remove(&t.id).is_some(),
                TombstoneOf::Entry => self.entries.remove(&t.id).is_some(),
            };
            report.deleted += usize::from(was_live);
            self.tombstones.insert(t.id, t);
        }
        if let Some(plan) = plan_in {
            self.plan = plan;
        }
        if let Some(settings) = settings_in {
            self.settings = settings;
        }

        report.changed += self.join_tags_with_the_same_name(now)?;
        let _ = self.remove_orphan_voices();
        Ok(report)
    }

    /// Keeps an entry's recording: already here, copied from the other
    /// device's file, or dropped if that file has no valid one.
    fn receive_voice(
        &mut self,
        entry: &mut Entry,
        voice: &dyn Fn(&Id) -> Option<Zeroizing<Vec<u8>>>,
    ) -> Result<(), JournalError> {
        let Some(note) = entry.voice.as_ref() else {
            return Ok(());
        };
        if self.vault.media_ids()?.contains(note.id.as_bytes()) {
            return Ok(());
        }
        match voice(&note.id).filter(|bytes| inspect_ogg_opus(bytes).is_ok()) {
            Some(bytes) => {
                self.vault
                    .put_media(*note.id.as_bytes(), RecordType::Voice, &bytes)?;
            }
            None => entry.voice = None,
        }
        Ok(())
    }

    /// Two devices may each create a tag with the same name. The one with the
    /// smaller id stays and takes over the other's entries, the same choice on
    /// every device. Returns how many tags were joined.
    fn join_tags_with_the_same_name(&mut self, now: OffsetDateTime) -> Result<usize, JournalError> {
        let mut tags: Vec<(Id, String)> = self
            .tags
            .values()
            .map(|t| (t.id, t.name.trim().to_lowercase()))
            .collect();
        tags.sort();
        let mut joined = 0;
        let mut kept: Vec<(String, Id)> = Vec::new();
        for (id, name) in tags {
            match kept.iter().find(|(n, _)| *n == name) {
                Some((_, keep)) => {
                    let keep = *keep;
                    self.merge_tags(&id, &keep, now)?;
                    joined += 1;
                }
                None => kept.push((name, id)),
            }
        }
        Ok(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minute: u8) -> Option<Timestamp> {
        Some(Timestamp(
            time::macros::datetime!(2026-10-22 10:00 UTC) + time::Duration::minutes(minute.into()),
        ))
    }

    fn live(minute: u8, bytes: &[u8]) -> Version {
        Version::Live(at(minute), Zeroizing::new(bytes.to_vec()))
    }

    fn dead(minute: u8) -> Version {
        Version::Dead(at(minute).unwrap())
    }

    #[test]
    fn the_newer_change_wins() {
        assert!(remote_wins(None, &live(0, b"a")));
        assert!(remote_wins(Some(&live(0, b"a")), &live(1, b"a")));
        assert!(!remote_wins(Some(&live(1, b"a")), &live(0, b"b")));
        assert!(remote_wins(Some(&live(0, b"a")), &dead(1)));
        assert!(!remote_wins(Some(&dead(1)), &live(0, b"a")));
        // An edit made after the deletion brings the record back.
        assert!(remote_wins(Some(&dead(0)), &live(1, b"a")));
        // Records from before sync count as oldest.
        assert!(remote_wins(
            Some(&Version::Live(None, Zeroizing::new(vec![]))),
            &live(0, b"")
        ));
    }

    #[test]
    fn ties_are_decided_the_same_way_on_every_device() {
        // Same time: a deletion beats an edit, whichever side holds it.
        assert!(remote_wins(Some(&live(0, b"a")), &dead(0)));
        assert!(!remote_wins(Some(&dead(0)), &live(0, b"a")));
        assert!(!remote_wins(Some(&dead(0)), &dead(0)));
        // Two different edits: the larger encoding wins on both devices.
        assert!(remote_wins(Some(&live(0, b"a")), &live(0, b"b")));
        assert!(!remote_wins(Some(&live(0, b"b")), &live(0, b"a")));
        // The same edit changes nothing.
        assert!(!remote_wins(Some(&live(0, b"a")), &live(0, b"a")));
    }
}

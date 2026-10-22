//! Deletions that travel to the user's other devices (sync, decision D-22).
//!
//! Deleting an entry or a tag replaces its record with a tombstone at the same
//! row id: the content is gone, and what is left says only what kind of record
//! it was and when it was deleted, encrypted like any record.

use core::fmt;

use serde::{Deserialize, Serialize};

use super::{Id, Timestamp};

/// What kind of record a tombstone replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TombstoneOf {
    Entry,
    Tag,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    /// The deleted record's id, equal to the tombstone's row id.
    pub id: Id,
    pub of: TombstoneOf,
    /// By the deleting device's clock; an edit made later elsewhere wins over it.
    pub deleted_at: Timestamp,
}

impl fmt::Debug for Tombstone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tombstone")
            .field("id", &self.id)
            .field("of", &self.of)
            .finish_non_exhaustive()
    }
}

//! Domain types (SPEC §5). They exist only in memory after unlock or as
//! encrypted CBOR blobs; nothing here touches the disk directly.
//!
//! Every type holding user text derives `ZeroizeOnDrop` and has a redacted
//! `Debug`, so content is wiped when the vault locks and never reaches logs.

pub mod entry;
pub mod plan;
pub mod settings;
pub mod tag;
pub mod tombstone;
pub mod voice;

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::{CryptoError, random_bytes};

pub use entry::{
    Emotion, Entry, EntryDraft, EntryKind, EntryKindTag, InboxEntry, MAX_DISCUSSED, MAX_TODOS,
    Symptom, Todo,
};
pub use plan::{Contact, SafetyPlan};
pub use settings::{AutoLock, Reminder, Settings, UnsavedEntry};
pub use tag::Tag;
pub use tombstone::{Tombstone, TombstoneOf};
pub use voice::{MAX_VOICE_BYTES, MAX_VOICE_MS, VOICE_MIME, VoiceNote, inspect_ogg_opus};

/// Domain-level error. Names the offending field, never its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainError {
    /// A field is outside its allowed range or format.
    InvalidField(&'static str),
    /// A referenced object does not exist.
    NotFound,
    /// A record could not be encoded or decoded.
    Encoding,
    Rng,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField(field) => write!(f, "invalid value in field `{field}`"),
            Self::NotFound => f.write_str("not found"),
            Self::Encoding => f.write_str("record encoding error"),
            Self::Rng => f.write_str("system random number generator unavailable"),
        }
    }
}

impl std::error::Error for DomainError {}

impl From<CryptoError> for DomainError {
    fn from(_: CryptoError) -> Self {
        Self::Rng
    }
}

/// A point in time with its UTC offset, serialized as an RFC 3339 string so the
/// stored format is self-describing and readable by other tools.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub OffsetDateTime);

impl Timestamp {
    pub fn into_inner(self) -> OffsetDateTime {
        self.0
    }
}

impl From<OffsetDateTime> for Timestamp {
    fn from(t: OffsetDateTime) -> Self {
        Self(t)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Timestamps are user data (when something happened); keep them out of logs.
        f.write_str("Timestamp([REDACTED])")
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let text = self.0.format(&Rfc3339).map_err(serde::ser::Error::custom)?;
        s.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = Zeroizing::new(String::deserialize(d)?);
        OffsetDateTime::parse(&text, &Rfc3339)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl zeroize::DefaultIsZeroes for Timestamp {}

impl Default for Timestamp {
    fn default() -> Self {
        Self(OffsetDateTime::UNIX_EPOCH)
    }
}

/// Identifier of a domain object (UUID, stored as 16 bytes in `records.id`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub Uuid);

impl Id {
    /// New random id (UUID v4). Ids are stored in plain text as row keys, so
    /// they must not carry a creation time as UUIDv7 would (decision D-13).
    pub fn new_random() -> Result<Self, DomainError> {
        let mut bytes = [0u8; 16];
        random_bytes(&mut bytes)?;
        Ok(Self(uuid::Builder::from_random_bytes(bytes).into_uuid()))
    }

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Ids are random and carry no content or time.
        write!(f, "Id({})", self.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl zeroize::DefaultIsZeroes for Id {}

impl Default for Id {
    fn default() -> Self {
        Self(Uuid::nil())
    }
}

/// Encodes a domain value as CBOR into a buffer that is zeroed on drop.
pub fn encode<T: Serialize>(value: &T) -> Result<Zeroizing<Vec<u8>>, DomainError> {
    let mut buf = Zeroizing::new(Vec::new());
    ciborium::into_writer(value, &mut *buf).map_err(|_| DomainError::Encoding)?;
    Ok(buf)
}

/// Decodes a domain value from CBOR.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DomainError> {
    ciborium::from_reader(bytes).map_err(|_| DomainError::Encoding)
}

/// Checks an optional rating against an inclusive range.
fn check_range(
    value: Option<u8>,
    range: core::ops::RangeInclusive<u8>,
    field: &'static str,
) -> Result<(), DomainError> {
    match value {
        Some(v) if !range.contains(&v) => Err(DomainError::InvalidField(field)),
        _ => Ok(()),
    }
}

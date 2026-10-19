//! In-memory queries over decrypted entries (SPEC §7: no plaintext search
//! columns on disk, all filtering happens in Rust after unlock).

pub mod filter;
pub mod search;
pub mod stats;

pub use filter::EntryFilter;
pub use search::SearchQuery;
pub use stats::{MoodPoint, StatsRange};

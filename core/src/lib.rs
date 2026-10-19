//! Mossbook core: domain logic, cryptography and storage.
//!
//! This crate does not depend on Tauri and is tested on its own (SPEC §8.2).

#![forbid(unsafe_code)]

pub mod crisis;
pub mod crypto;
pub mod domain;
pub mod journal;
pub mod query;
pub mod store;
pub mod vault;

/// Version of the `mossbook-core` crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(VERSION, "0.1.0");
    }
}

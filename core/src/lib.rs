//! Mossbook core: domain logic, cryptography and storage.
//!
//! This crate does not depend on Tauri and is tested on its own (SPEC §8.2).

#![forbid(unsafe_code)]

pub mod backup;
pub mod crisis;
pub mod crypto;
pub mod domain;
pub mod journal;
pub mod query;
pub mod report;
pub mod store;
pub mod transcribe;
pub mod vault;

/// Version of the `mossbook-core` crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        // The release version itself is checked across Cargo, tauri.conf.json and
        // package.json by the app crate's `versions_match` test.
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()), "{VERSION}");
    }
}

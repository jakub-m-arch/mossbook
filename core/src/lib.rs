//! Mossbook core: logika domenowa, kryptografia i przechowywanie.
//!
//! Crate nie zależy od Tauri i jest testowany niezależnie (SPEC §8.2).

#![forbid(unsafe_code)]

/// Wersja crate'a `mossbook-core`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(VERSION, "0.1.0");
    }
}

//! Crisis phone numbers (P-3, SPEC §9).
//!
//! Static data shipped with the app, one file per country in `core/data/crisis/`,
//! each with its sources and verification date. It is the only content shown
//! without unlocking and never comes from the user.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Files compiled into the binary, by ISO 3166-1 alpha-2 code.
const FILES: &[(&str, &str)] = &[("PL", include_str!("../data/crisis/pl.json"))];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrisisDirectory {
    pub country: String,
    /// Date the numbers were last checked against the sources (`YYYY-MM-DD`).
    pub verified_at: String,
    pub sources: Vec<String>,
    pub lines: Vec<CrisisLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrisisLine {
    /// As displayed, e.g. `800 70 2222`.
    pub number: String,
    /// The general emergency number (shown first and highlighted).
    pub emergency: bool,
    pub hours: String,
    pub free: bool,
    /// Translations by UI locale (`pl`, `en`, `uk`).
    pub name: BTreeMap<String, String>,
    pub description: BTreeMap<String, String>,
}

impl CrisisLine {
    /// Digits only, for dialing.
    pub fn dial(&self) -> String {
        self.number.chars().filter(char::is_ascii_digit).collect()
    }
}

/// Country codes with crisis data.
pub fn countries() -> Vec<&'static str> {
    FILES.iter().map(|(code, _)| *code).collect()
}

/// Crisis numbers for a country (case-insensitive code).
pub fn directory(country: &str) -> Option<CrisisDirectory> {
    FILES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(country))
        .map(|(_, json)| serde_json::from_str(json).expect("crisis data is validated by tests"))
}

/// Whether `number` (digits only) appears in any crisis directory. Dialing is
/// restricted to these numbers and to the user's own safety-plan contacts.
pub fn is_known_number(number: &str) -> bool {
    FILES
        .iter()
        .filter_map(|(code, _)| directory(code))
        .any(|d| d.lines.iter().any(|l| l.dial() == number))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCALES: [&str; 3] = ["pl", "en", "uk"];

    /// Every shipped file must be complete; this is what makes `directory` infallible.
    #[test]
    fn all_files_are_valid() {
        for (code, json) in FILES {
            let d: CrisisDirectory =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{code}: {e}"));
            assert_eq!(&d.country, code);
            assert!(
                time::Date::parse(
                    &d.verified_at,
                    &time::macros::format_description!("[year]-[month]-[day]")
                )
                .is_ok()
            );
            assert!(
                !d.sources.is_empty() && d.sources.iter().all(|s| s.starts_with("https://")),
                "{code}"
            );
            assert!(
                d.lines.iter().filter(|l| l.emergency).count() == 1,
                "{code}: exactly one emergency number"
            );
            for line in &d.lines {
                assert!(
                    line.number.chars().all(|c| c.is_ascii_digit() || c == ' '),
                    "{}",
                    line.number
                );
                assert!(line.dial().len() >= 3);
                for locale in LOCALES {
                    assert!(
                        line.name.get(locale).is_some_and(|s| !s.is_empty()),
                        "{code} {} {locale}",
                        line.number
                    );
                    assert!(line.description.get(locale).is_some_and(|s| !s.is_empty()));
                }
            }
        }
    }

    #[test]
    fn poland_matches_the_spec() {
        let pl = directory("pl").unwrap();
        let numbers: Vec<String> = pl.lines.iter().map(CrisisLine::dial).collect();
        assert_eq!(numbers, ["112", "116123", "800702222", "116111"]);
        assert!(pl.lines[0].emergency);
        assert_eq!(countries(), ["PL"]);
        assert!(directory("XX").is_none());
    }

    #[test]
    fn known_numbers() {
        assert!(is_known_number("116123"));
        assert!(!is_known_number("116 123"));
        assert!(!is_known_number("0700123456"));
    }
}

//! User settings (SPEC §5). Encrypted like everything else; see decision D-2 for
//! what the lock screen uses before they can be read.

use core::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{DomainError, EntryDraft, Timestamp};

/// Supported UI languages (SPEC §4.7).
pub const LOCALES: [&str; 3] = ["pl", "en", "uk"];

// Field-level `serde(default)`: a container-level one would move out of a `Drop` type.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Settings {
    #[serde(default)]
    #[zeroize(skip)]
    pub autolock: AutoLock,
    /// UI language; `None` follows the system language (decision D-2).
    /// Older records stored a plain string, which still decodes as `Some`.
    #[serde(default)]
    pub locale: Option<String>,
    /// ISO 3166-1 alpha-2 country of the crisis numbers, e.g. `PL`.
    #[serde(default = "default_crisis_country")]
    pub crisis_country: String,
    #[serde(default)]
    pub reminders: Vec<Reminder>,
    /// Automatic local backups when the journal locks (decision D-16).
    #[serde(default = "default_true")]
    #[zeroize(skip)]
    pub auto_backup: bool,
    /// When the user last saved a backup themselves. Set by the journal, never
    /// by the UI (see `Journal::save_settings`).
    #[serde(default)]
    pub last_export: Option<Timestamp>,
    /// The entry form that was open when the journal locked (decision D-20).
    /// Set by the journal, never by the UI (see `Journal::keep_unsaved_entry`).
    /// Left out when empty, so settings without one look as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsaved_entry: Option<UnsavedEntry>,
    /// When this was last changed, by the device's clock (sync, D-22). Set by
    /// the journal; `None` in records from before sync, which count as oldest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
}

/// An entry form kept while it is being filled in, so a lock does not lose it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
pub struct UnsavedEntry {
    pub draft: EntryDraft,
    /// When the form was last kept.
    pub kept_at: Timestamp,
}

impl fmt::Debug for UnsavedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsavedEntry")
            .field("draft", &self.draft)
            .finish_non_exhaustive()
    }
}

fn default_true() -> bool {
    true
}

fn default_crisis_country() -> String {
    "PL".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            autolock: AutoLock::default(),
            locale: None,
            crisis_country: default_crisis_country(),
            reminders: Vec::new(),
            auto_backup: true,
            last_export: None,
            unsaved_entry: None,
            updated_at: None,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self
            .locale
            .as_deref()
            .is_some_and(|l| !LOCALES.contains(&l))
        {
            return Err(DomainError::InvalidField("locale"));
        }
        let c = self.crisis_country.as_bytes();
        if c.len() != 2 || !c.iter().all(u8::is_ascii_uppercase) {
            return Err(DomainError::InvalidField("crisis_country"));
        }
        self.reminders.iter().try_for_each(Reminder::validate)
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("autolock", &self.autolock)
            .finish_non_exhaustive()
    }
}

/// Auto-lock policy (V-3). On desktop, losing focus only covers the window and
/// hiding/minimizing locks (decision D-11); the timer applies everywhere.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoLock {
    /// Lock as soon as the app goes to the background.
    #[default]
    Immediately,
    After1Min,
    After5Min,
    After15Min,
}

impl AutoLock {
    /// Inactivity timeout, `None` for [`AutoLock::Immediately`].
    pub fn timeout(self) -> Option<std::time::Duration> {
        let minutes = match self {
            Self::Immediately => return None,
            Self::After1Min => 1,
            Self::After5Min => 5,
            Self::After15Min => 15,
        };
        Some(std::time::Duration::from_secs(minutes * 60))
    }
}

/// Local reminder (R-1, v1). Its text is shown on the lock screen, so it is
/// written by the user and never derived from entries (R-2).
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
pub struct Reminder {
    pub hour: u8,
    pub minute: u8,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub enabled: bool,
}

impl Reminder {
    fn validate(&self) -> Result<(), DomainError> {
        if self.hour > 23 || self.minute > 59 {
            return Err(DomainError::InvalidField("reminder_time"));
        }
        Ok(())
    }
}

impl fmt::Debug for Reminder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reminder")
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{decode, encode};

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let s = Settings::default();
        assert_eq!(s.validate(), Ok(()));
        assert_eq!(s.autolock, AutoLock::Immediately);
        assert_eq!(decode::<Settings>(&encode(&s).unwrap()).unwrap(), s);
    }

    #[test]
    fn invalid_values_are_rejected() {
        let mut s = Settings::default();
        s.locale = Some("de".into());
        assert_eq!(s.validate(), Err(DomainError::InvalidField("locale")));
        s.locale = Some("uk".into());
        s.crisis_country = "pl".into();
        assert_eq!(
            s.validate(),
            Err(DomainError::InvalidField("crisis_country"))
        );
        s.crisis_country = "UA".into();
        s.reminders.push(Reminder {
            hour: 24,
            minute: 0,
            text: String::new(),
            enabled: false,
        });
        assert_eq!(
            s.validate(),
            Err(DomainError::InvalidField("reminder_time"))
        );
    }

    #[test]
    fn older_records_with_a_plain_locale_still_decode() {
        #[derive(serde::Serialize)]
        struct V1<'a> {
            autolock: &'a str,
            locale: &'a str,
            crisis_country: &'a str,
        }
        let bytes = encode(&V1 {
            autolock: "after5_min",
            locale: "pl",
            crisis_country: "PL",
        })
        .unwrap();
        let s: Settings = decode(&bytes).unwrap();
        assert_eq!(s.locale.as_deref(), Some("pl"));
        assert_eq!(s.autolock, AutoLock::After5Min);
        assert_eq!(Settings::default().locale, None);
        // Fields added later get their defaults.
        assert!(s.auto_backup);
        assert_eq!(s.last_export, None);
    }

    #[test]
    fn autolock_timeouts() {
        assert_eq!(AutoLock::Immediately.timeout(), None);
        assert_eq!(AutoLock::After5Min.timeout().unwrap().as_secs(), 300);
    }

    #[test]
    fn debug_hides_reminder_text() {
        let mut s = Settings::default();
        s.reminders = vec![Reminder {
            hour: 20,
            minute: 0,
            text: "time for yourself".into(),
            enabled: true,
        }];
        assert!(!format!("{s:?}").contains("yourself"));
    }
}

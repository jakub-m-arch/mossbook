//! Tags (E-12).

use core::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{DomainError, Id, Timestamp};

/// Maximum tag name length in characters.
pub const MAX_NAME_CHARS: usize = 64;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Tag {
    pub id: Id,
    pub name: String,
    /// Color as `#rrggbb`.
    pub color: String,
    /// When this was last changed, by the device's clock (sync, D-22). Set by
    /// the journal; `None` in records from before sync, which count as oldest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
}

impl Tag {
    /// Trims the name and checks name and color.
    pub fn normalize(&mut self) -> Result<(), DomainError> {
        let trimmed = self.name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > MAX_NAME_CHARS {
            return Err(DomainError::InvalidField("name"));
        }
        if trimmed.len() != self.name.len() {
            self.name = trimmed.to_owned();
        }
        if !is_hex_color(&self.color) {
            return Err(DomainError::InvalidField("color"));
        }
        self.color.make_ascii_lowercase();
        Ok(())
    }

    /// Case-insensitive name comparison used to prevent duplicate tags.
    pub fn same_name(&self, other: &str) -> bool {
        self.name.trim().to_lowercase() == other.trim().to_lowercase()
    }
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tag")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, color: &str) -> Tag {
        Tag {
            id: Id::default(),
            name: name.into(),
            color: color.into(),
            updated_at: None,
        }
    }

    #[test]
    fn normalize_trims_and_validates() {
        let mut t = tag("  work  ", "#A0B1C2");
        t.normalize().unwrap();
        assert_eq!(t.name, "work");
        assert_eq!(t.color, "#a0b1c2");
        assert_eq!(
            tag("   ", "#000000").normalize(),
            Err(DomainError::InvalidField("name"))
        );
        assert_eq!(
            tag(&"x".repeat(65), "#000000").normalize(),
            Err(DomainError::InvalidField("name"))
        );
        for bad in ["000000", "#00000", "#0000000", "#gggggg", "red"] {
            assert_eq!(
                tag("a", bad).normalize(),
                Err(DomainError::InvalidField("color")),
                "{bad}"
            );
        }
    }

    #[test]
    fn same_name_ignores_case_and_spaces() {
        assert!(tag("Praca", "#000000").same_name(" praca "));
        assert!(!tag("Praca", "#000000").same_name("sen"));
    }

    #[test]
    fn debug_hides_name() {
        assert!(!format!("{:?}", tag("therapy", "#000000")).contains("therapy"));
    }
}

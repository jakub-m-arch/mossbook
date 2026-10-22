//! "Plan for a hard moment" (P-1): lists and contacts the user writes in their own words.

use core::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::Timestamp;

// Field-level `serde(default)`: a container-level one would move out of a `Drop` type.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SafetyPlan {
    #[serde(default)]
    pub warning_signs: Vec<String>,
    #[serde(default)]
    pub self_coping: Vec<String>,
    #[serde(default)]
    pub calming_people_places: Vec<String>,
    #[serde(default)]
    pub contacts: Vec<Contact>,
    #[serde(default)]
    pub professionals: Vec<Contact>,
    #[serde(default)]
    pub safe_environment: Vec<String>,
    /// When this was last changed, by the device's clock (sync, D-22). Set by
    /// the journal; `None` in records from before sync, which count as oldest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
}

impl SafetyPlan {
    /// Drops empty lines and contacts with neither a name nor a phone number.
    pub fn normalize(&mut self) {
        for list in [
            &mut self.warning_signs,
            &mut self.self_coping,
            &mut self.calming_people_places,
            &mut self.safe_environment,
        ] {
            list.retain(|item| !item.trim().is_empty());
        }
        for list in [&mut self.contacts, &mut self.professionals] {
            list.retain(|c| !c.name.trim().is_empty() || !c.phone.trim().is_empty());
        }
    }
}

impl fmt::Debug for SafetyPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SafetyPlan").finish_non_exhaustive()
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Contact {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub note: String,
}

impl fmt::Debug for Contact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Contact").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{decode, encode};

    fn contact(name: &str, note: &str) -> Contact {
        let mut c = Contact::default();
        c.name = name.into();
        c.note = note.into();
        c
    }

    #[test]
    fn normalize_drops_empty_items() {
        let mut plan = SafetyPlan::default();
        plan.warning_signs = vec!["can't sleep".into(), "  ".into()];
        plan.contacts = vec![contact("Ola", ""), contact("", "only a note")];
        plan.normalize();
        assert_eq!(plan.warning_signs, vec!["can't sleep".to_string()]);
        assert_eq!(plan.contacts.len(), 1);
    }

    #[test]
    fn round_trip_and_debug() {
        let mut plan = SafetyPlan::default();
        plan.self_coping = vec!["walk the dog".into()];
        assert_eq!(decode::<SafetyPlan>(&encode(&plan).unwrap()).unwrap(), plan);
        assert!(!format!("{plan:?}").contains("dog"));
    }
}

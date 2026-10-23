//! What has helped (D-27): the strategies written in fear entries and what
//! helped in event entries, grouped by their wording, over the whole journal.
//! Counts and averages only, never a ranking of what works (SPEC §4.4); the UI
//! words them as observations.

use core::fmt;
use std::collections::HashMap;

use serde::Serialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::domain::{Entry, EntryKind, Timestamp};

use super::search::fold;

/// Longest list returned.
pub const MAX_HELPS: usize = 50;

/// One thing that helped, as the user wrote it.
#[derive(Clone, PartialEq, Serialize, Zeroize, ZeroizeOnDrop)]
pub struct Help {
    /// The most common spelling (the latest one on a tie).
    pub label: String,
    /// Entries that mention it.
    pub count: u32,
    /// When it was last written down.
    #[zeroize(skip)]
    pub last: Timestamp,
    /// Average of `intensity - intensity_after` in fear entries that have both;
    /// positive means the fear went down. `None` without such an entry.
    pub average_drop: Option<f32>,
    /// Fear entries behind `average_drop`.
    pub measured: u32,
}

impl fmt::Debug for Help {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Help")
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Group {
    count: u32,
    last: Option<Timestamp>,
    drop: i32,
    measured: u32,
    /// Spelling → (uses, latest use).
    spellings: HashMap<String, (u32, Timestamp)>,
}

/// What has helped, most often written first, then most recent.
pub fn helps<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> Vec<Help> {
    let mut groups: HashMap<String, Group> = HashMap::new();
    for e in entries {
        let (text, change) = match &e.kind {
            EntryKind::Fear {
                strategy: Some(s),
                intensity_after,
                ..
            } => (
                s.as_str(),
                e.intensity
                    .zip(*intensity_after)
                    .map(|(before, after)| i32::from(before) - i32::from(after)),
            ),
            EntryKind::Event { helped, .. } => (helped.as_str(), None),
            _ => continue,
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let key = fold(text).split_whitespace().collect::<Vec<_>>().join(" ");
        let g = groups.entry(key).or_default();
        g.count += 1;
        g.last = g.last.max(Some(e.occurred_at));
        if let Some(change) = change {
            g.drop += change;
            g.measured += 1;
        }
        let spelling = g
            .spellings
            .entry(text.to_owned())
            .or_insert((0, e.occurred_at));
        spelling.0 += 1;
        spelling.1 = spelling.1.max(e.occurred_at);
    }
    let mut out: Vec<Help> = groups
        .into_values()
        .filter_map(|g| {
            let label = g
                .spellings
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                .map(|(s, _)| s.clone())?;
            Some(Help {
                label,
                count: g.count,
                last: g.last?,
                average_drop: (g.measured > 0).then(|| g.drop as f32 / g.measured as f32),
                measured: g.measured,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.last.cmp(&a.last))
            .then_with(|| a.label.cmp(&b.label))
    });
    out.truncate(MAX_HELPS);
    out
}

#[cfg(test)]
mod tests {
    use time::Duration;
    use time::macros::datetime;

    use super::*;
    use crate::domain::EntryKindTag;

    const NOW: time::OffsetDateTime = datetime!(2026-10-19 12:00 UTC);

    fn fear(days_ago: i64, strategy: &str, before: Option<u8>, after: Option<u8>) -> Entry {
        let mut e = Entry::new(
            EntryKind::Fear {
                trigger: String::new(),
                symptoms: Vec::new(),
                avoidance: None,
                strategy: Some(strategy.into()),
                intensity_after: after,
            },
            NOW,
        )
        .unwrap();
        e.intensity = before;
        e.occurred_at = (NOW - Duration::days(days_ago)).into();
        e
    }

    fn event(days_ago: i64, helped: &str) -> Entry {
        let mut e = Entry::new(EntryKind::empty(EntryKindTag::Event), NOW).unwrap();
        if let EntryKind::Event { helped: h, .. } = &mut e.kind {
            *h = helped.into();
        }
        e.occurred_at = (NOW - Duration::days(days_ago)).into();
        e
    }

    #[test]
    fn groups_by_wording_and_averages_the_change_in_fear() {
        let entries = [
            fear(3, "Oddychanie 4-7-8", Some(8), Some(4)),
            fear(2, "oddychanie  4-7-8", Some(6), Some(4)),
            // No intensity after: counted, not measured.
            fear(1, "Oddychanie 4-7-8", Some(7), None),
            event(5, "Spacer"),
            event(4, " spacer "),
            event(0, "rozmowa z siostrą"),
        ];
        let out = helps(&entries);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].label, "Oddychanie 4-7-8");
        assert_eq!(out[0].count, 3);
        assert_eq!(out[0].measured, 2);
        assert_eq!(out[0].average_drop, Some(3.0));
        assert_eq!(out[0].last, Timestamp(NOW - Duration::days(1)));
        // Same count: the latest spelling wins the tie; never measured.
        assert_eq!(out[1].label, "spacer");
        assert_eq!(out[1].count, 2);
        assert_eq!(out[1].average_drop, None);
        assert_eq!(out[2].label, "rozmowa z siostrą");
    }

    #[test]
    fn empty_texts_and_other_types_are_left_out() {
        let mut note = Entry::new(EntryKind::Note, NOW).unwrap();
        note.note = "spacer".into();
        let entries = [fear(0, "   ", Some(5), Some(2)), event(0, ""), note];
        assert!(helps(&entries).is_empty());
    }

    #[test]
    fn the_list_is_capped() {
        let entries: Vec<Entry> = (0..MAX_HELPS as i64 + 5)
            .map(|i| event(i, &format!("synthetic {i}")))
            .collect();
        let out = helps(&entries);
        assert_eq!(out.len(), MAX_HELPS);
        assert_eq!(
            out[0].label, "synthetic 0",
            "most recent first among equals"
        );
    }
}

//! Mood over time (A-1). Plain observations, no clinical interpretation (SPEC §4.4).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::{Date, Duration, OffsetDateTime};

use crate::domain::Entry;

/// Time windows offered by the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatsRange {
    Week,
    Month,
    ThreeMonths,
}

impl StatsRange {
    /// `(from, to)` ending at `now`, `from` inclusive.
    pub fn bounds(self, now: OffsetDateTime) -> (OffsetDateTime, OffsetDateTime) {
        let days = match self {
            Self::Week => 7,
            Self::Month => 30,
            Self::ThreeMonths => 90,
        };
        (now - Duration::days(days), now)
    }
}

/// Average mood of one calendar day.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MoodPoint {
    pub date: Date,
    pub average: f32,
    pub count: u32,
}

/// Daily average of `mood` (1..=5) over entries in `[from, to]`, oldest first.
///
/// Any entry type with a mood rating counts. The day is the calendar date in the
/// entry's own UTC offset, i.e. where the user was when it happened.
pub fn mood_by_day<'a>(
    entries: impl IntoIterator<Item = &'a Entry>,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> Vec<MoodPoint> {
    let mut days: BTreeMap<Date, (u32, u32)> = BTreeMap::new();
    for e in entries {
        let at = e.occurred_at.0;
        if let (Some(mood), true) = (e.mood, at >= from && at <= to) {
            let day = days.entry(at.date()).or_default();
            day.0 += u32::from(mood);
            day.1 += 1;
        }
    }
    days.into_iter()
        .map(|(date, (sum, count))| MoodPoint {
            date,
            average: sum as f32 / count as f32,
            count,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntryKind, EntryKindTag};
    use time::macros::{date, datetime};

    fn mood(at: OffsetDateTime, value: Option<u8>) -> Entry {
        let mut e = Entry::new(EntryKind::empty(EntryKindTag::Mood), at).unwrap();
        e.mood = value;
        e
    }

    #[test]
    fn averages_per_local_day() {
        let entries = [
            mood(datetime!(2026-10-18 09:00 +02:00), Some(2)),
            mood(datetime!(2026-10-18 21:00 +02:00), Some(4)),
            // 23:30 local on the 18th is already the 19th in UTC: still counts for the 18th.
            mood(datetime!(2026-10-18 23:30 +02:00), Some(3)),
            mood(datetime!(2026-10-19 08:00 +02:00), Some(5)),
            mood(datetime!(2026-10-19 09:00 +02:00), None),
            mood(datetime!(2026-09-01 09:00 +02:00), Some(1)), // outside range
        ];
        let points = mood_by_day(
            &entries,
            datetime!(2026-10-12 00:00 UTC),
            datetime!(2026-10-20 00:00 UTC),
        );
        assert_eq!(
            points,
            vec![
                MoodPoint {
                    date: date!(2026 - 10 - 18),
                    average: 3.0,
                    count: 3
                },
                MoodPoint {
                    date: date!(2026 - 10 - 19),
                    average: 5.0,
                    count: 1
                },
            ]
        );
    }

    #[test]
    fn range_bounds() {
        let now = datetime!(2026-10-19 12:00 UTC);
        assert_eq!(
            StatsRange::Week.bounds(now).0,
            datetime!(2026-10-12 12:00 UTC)
        );
        assert_eq!(
            StatsRange::ThreeMonths.bounds(now).0,
            datetime!(2026-07-21 12:00 UTC)
        );
    }
}

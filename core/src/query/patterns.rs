//! Patterns over a period (A-2, A-3, A-4). Plain counts and averages, never
//! interpretations; the UI words them as observations (SPEC §4.4).
//!
//! Nothing is computed below [`MIN_ENTRIES`] entries in the period.

use std::collections::HashMap;

use serde::Serialize;
use time::OffsetDateTime;

use crate::domain::{Entry, EntryKind, Id, Tag};

use super::search::fold;

/// Below this many entries in the period no patterns are shown (SPEC §4.4).
pub const MIN_ENTRIES: usize = 14;
/// Free-text groups (triggers, strategies) need at least this many occurrences.
pub const MIN_REPEATS: u32 = 2;
/// Longest lists returned.
const TOP: usize = 5;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Patterns {
    /// Entries in the period.
    pub entries: usize,
    /// `false` below [`MIN_ENTRIES`]; then every list below is empty.
    pub enough_data: bool,
    pub top_tags: Vec<Count>,
    pub top_triggers: Vec<Count>,
    /// Mood on days after a night with poor (1–2), fair (3) or good (4–5) sleep.
    pub mood_by_sleep: Vec<Group>,
    pub mood_by_time_of_day: Vec<Group>,
    /// Monday first.
    pub mood_by_weekday: Vec<Group>,
    pub strategies: Vec<Strategy>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Count {
    /// Tag id for tags; the most common original spelling for free text.
    pub key: String,
    pub label: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    /// Stable key for translation, e.g. `poor`, `morning`, `monday`.
    pub key: &'static str,
    pub average: f32,
    /// Ratings (or days, for sleep) behind the average.
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Strategy {
    pub label: String,
    /// Average of `intensity - intensity_after`; positive means it went down.
    pub average_drop: f32,
    pub count: u32,
}

/// Computes all patterns for entries whose `occurred_at` is in `[from, to]`.
pub fn patterns<'a>(
    entries: impl IntoIterator<Item = &'a Entry>,
    tags: &HashMap<Id, Tag>,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> Patterns {
    let entries: Vec<&Entry> = entries
        .into_iter()
        .filter(|e| e.occurred_at.0 >= from && e.occurred_at.0 <= to)
        .collect();
    let mut p = Patterns {
        entries: entries.len(),
        enough_data: entries.len() >= MIN_ENTRIES,
        top_tags: Vec::new(),
        top_triggers: Vec::new(),
        mood_by_sleep: Vec::new(),
        mood_by_time_of_day: Vec::new(),
        mood_by_weekday: Vec::new(),
        strategies: Vec::new(),
    };
    if !p.enough_data {
        return p;
    }
    p.top_tags = top_tags(&entries, tags);
    p.top_triggers = top_free_text(entries.iter().filter_map(|e| match &e.kind {
        EntryKind::Fear { trigger, .. } => Some(trigger.as_str()),
        _ => None,
    }));
    p.mood_by_sleep = mood_by_sleep(&entries);
    p.mood_by_time_of_day = mood_by_time_of_day(&entries);
    p.mood_by_weekday = mood_by_weekday(&entries);
    p.strategies = strategies(&entries);
    p
}

fn top_tags(entries: &[&Entry], tags: &HashMap<Id, Tag>) -> Vec<Count> {
    let mut counts: HashMap<Id, u32> = HashMap::new();
    for e in entries {
        for id in &e.tags {
            *counts.entry(*id).or_default() += 1;
        }
    }
    let mut out: Vec<Count> = counts
        .into_iter()
        .filter_map(|(id, count)| {
            tags.get(&id).map(|t| Count {
                key: id.to_string(),
                label: t.name.clone(),
                count,
            })
        })
        .collect();
    sort_counts(&mut out);
    out
}

/// Groups free text by its folded form (case, diacritics, spacing), keeping the
/// most frequent original spelling as the label.
fn top_free_text<'a>(texts: impl Iterator<Item = &'a str>) -> Vec<Count> {
    let mut groups: HashMap<String, (u32, HashMap<&'a str, u32>)> = HashMap::new();
    for text in texts.map(str::trim).filter(|t| !t.is_empty()) {
        let key = fold(text).split_whitespace().collect::<Vec<_>>().join(" ");
        let group = groups.entry(key).or_default();
        group.0 += 1;
        *group.1.entry(text).or_default() += 1;
    }
    let mut out: Vec<Count> = groups
        .into_iter()
        .filter(|(_, (count, _))| *count >= MIN_REPEATS)
        .map(|(key, (count, spellings))| {
            let label = spellings
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
                .map(|(s, _)| s.to_owned())
                .unwrap_or_default();
            Count { key, label, count }
        })
        .collect();
    sort_counts(&mut out);
    out
}

fn sort_counts(v: &mut Vec<Count>) {
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    v.truncate(TOP);
}

/// Daily average mood keyed by local date.
fn daily_mood(entries: &[&Entry]) -> HashMap<time::Date, (u32, u32)> {
    let mut days: HashMap<time::Date, (u32, u32)> = HashMap::new();
    for e in entries {
        if let Some(m) = e.mood {
            let d = days.entry(e.occurred_at.0.date()).or_default();
            d.0 += u32::from(m);
            d.1 += 1;
        }
    }
    days
}

/// A sleep entry belongs to the day the person woke up: its `wake` time if set,
/// otherwise the day it was written down.
fn mood_by_sleep(entries: &[&Entry]) -> Vec<Group> {
    let moods = daily_mood(entries);
    // Per group: sum of daily averages (×100, to stay in integers) and number of days.
    let mut groups: [(u32, u32); 3] = [(0, 0); 3];
    for e in entries {
        let EntryKind::Sleep {
            quality: Some(q),
            wake,
            ..
        } = &e.kind
        else {
            continue;
        };
        let day = wake.map_or(e.occurred_at.0, |w| w.0).date();
        let Some((sum, n)) = moods.get(&day) else {
            continue;
        };
        let slot = match q {
            1 | 2 => 0,
            3 => 1,
            _ => 2,
        };
        groups[slot].0 += sum * 100 / n;
        groups[slot].1 += 1;
    }
    ["poor", "fair", "good"]
        .into_iter()
        .zip(groups)
        .filter(|(_, (_, days))| *days > 0)
        .map(|(key, (sum, days))| Group {
            key,
            average: sum as f32 / 100.0 / days as f32,
            count: days,
        })
        .collect()
}

fn group_moods(
    entries: &[&Entry],
    keys: &[&'static str],
    slot: impl Fn(&OffsetDateTime) -> usize,
) -> Vec<Group> {
    let mut sums = vec![(0u32, 0u32); keys.len()];
    for e in entries {
        if let Some(m) = e.mood {
            let s = &mut sums[slot(&e.occurred_at.0)];
            s.0 += u32::from(m);
            s.1 += 1;
        }
    }
    keys.iter()
        .zip(sums)
        .filter(|(_, (_, n))| *n > 0)
        .map(|(key, (sum, n))| Group {
            key,
            average: sum as f32 / n as f32,
            count: n,
        })
        .collect()
}

fn mood_by_time_of_day(entries: &[&Entry]) -> Vec<Group> {
    group_moods(
        entries,
        &["night", "morning", "afternoon", "evening"],
        |t| match t.hour() {
            0..=4 => 0,
            5..=11 => 1,
            12..=17 => 2,
            _ => 3,
        },
    )
}

fn mood_by_weekday(entries: &[&Entry]) -> Vec<Group> {
    group_moods(
        entries,
        &[
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
        ],
        |t| usize::from(t.weekday().number_days_from_monday()),
    )
}

fn strategies(entries: &[&Entry]) -> Vec<Strategy> {
    let mut groups: HashMap<String, (i32, u32, HashMap<String, u32>)> = HashMap::new();
    for e in entries {
        let EntryKind::Fear {
            strategy: Some(s),
            intensity_after: Some(after),
            ..
        } = &e.kind
        else {
            continue;
        };
        let (Some(before), s) = (e.intensity, s.trim()) else {
            continue;
        };
        if s.is_empty() {
            continue;
        }
        let key = fold(s).split_whitespace().collect::<Vec<_>>().join(" ");
        let g = groups.entry(key).or_default();
        g.0 += i32::from(before) - i32::from(*after);
        g.1 += 1;
        *g.2.entry(s.to_owned()).or_default() += 1;
    }
    let mut out: Vec<Strategy> = groups
        .into_values()
        .filter(|(_, n, _)| *n >= MIN_REPEATS)
        .map(|(drop, n, spellings)| Strategy {
            label: spellings
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(s, _)| s)
                .unwrap_or_default(),
            average_drop: drop as f32 / n as f32,
            count: n,
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    out.truncate(TOP);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntryKindTag, Timestamp};
    use time::Duration;
    use time::macros::datetime;

    const NOW: OffsetDateTime = datetime!(2026-10-19 20:00 +02:00);

    fn at(day: u8, hour: u8, kind: EntryKind, mood: Option<u8>) -> Entry {
        let mut e = Entry::new(kind, NOW).unwrap();
        e.occurred_at = Timestamp(
            datetime!(2026-10-01 00:00 +02:00)
                + Duration::days(i64::from(day) - 1)
                + Duration::hours(i64::from(hour)),
        );
        e.mood = mood;
        e
    }

    fn fear(day: u8, trigger: &str, strategy: &str, before: u8, after: u8) -> Entry {
        let mut e = at(
            day,
            15,
            EntryKind::Fear {
                trigger: trigger.into(),
                symptoms: vec![],
                avoidance: None,
                strategy: Some(strategy.into()),
                intensity_after: Some(after),
            },
            None,
        );
        e.intensity = Some(before);
        e
    }

    fn sleep(day: u8, quality: u8) -> Entry {
        at(
            day,
            7,
            EntryKind::Sleep {
                bedtime: None,
                wake: None,
                quality: Some(quality),
                awakenings: None,
                nightmare: false,
            },
            None,
        )
    }

    fn sample() -> Vec<Entry> {
        let mut v = vec![
            fear(1, "Tłum w tramwaju", "oddech pudełkowy", 8, 4),
            fear(2, "tlum  w tramwaju", "Oddech pudełkowy", 6, 3),
            fear(3, "zebranie", "spacer", 7, 6),
            fear(4, "zebranie", "spacer", 5, 5),
            fear(5, "telefon", "muzyka", 9, 2), // used once: not reported
            sleep(6, 1),
            sleep(7, 5),
            sleep(8, 3),
        ];
        // Moods: day 6 after poor sleep low, day 7 after good sleep high.
        v.push(at(6, 9, EntryKind::empty(EntryKindTag::Mood), Some(2)));
        v.push(at(6, 21, EntryKind::empty(EntryKindTag::Mood), Some(1)));
        v.push(at(7, 9, EntryKind::empty(EntryKindTag::Mood), Some(4)));
        v.push(at(7, 14, EntryKind::empty(EntryKindTag::Mood), Some(5)));
        v.push(at(8, 2, EntryKind::empty(EntryKindTag::Mood), Some(3)));
        v.push(at(9, 19, EntryKind::empty(EntryKindTag::Mood), Some(4)));
        v
    }

    fn range() -> (OffsetDateTime, OffsetDateTime) {
        (datetime!(2026-09-20 00:00 +02:00), NOW)
    }

    #[test]
    fn nothing_below_fourteen_entries() {
        let mut v = sample();
        v.truncate(13);
        let (from, to) = range();
        let p = patterns(&v, &HashMap::new(), from, to);
        assert_eq!((p.entries, p.enough_data), (13, false));
        assert!(
            p.top_triggers.is_empty() && p.strategies.is_empty() && p.mood_by_weekday.is_empty()
        );
    }

    #[test]
    fn triggers_and_strategies_group_folded_text_and_need_repeats() {
        let v = sample();
        let (from, to) = range();
        let p = patterns(&v, &HashMap::new(), from, to);
        assert!(p.enough_data);
        let triggers: Vec<(&str, u32)> = p
            .top_triggers
            .iter()
            .map(|c| (c.label.as_str(), c.count))
            .collect();
        assert_eq!(triggers.len(), 2);
        assert!(triggers.contains(&("zebranie", 2)));
        assert!(
            triggers
                .iter()
                .any(|(l, n)| l.to_lowercase().contains("tramwaju") && *n == 2)
        );
        let strategies: Vec<(&str, f32, u32)> = p
            .strategies
            .iter()
            .map(|s| (s.label.as_str(), s.average_drop, s.count))
            .collect();
        assert_eq!(strategies.len(), 2, "single-use 'muzyka' is left out");
        let breathing = p
            .strategies
            .iter()
            .find(|s| s.label.to_lowercase().contains("oddech"))
            .unwrap();
        assert_eq!((breathing.average_drop, breathing.count), (3.5, 2));
        let walk = p.strategies.iter().find(|s| s.label == "spacer").unwrap();
        assert_eq!(walk.average_drop, 0.5);
    }

    #[test]
    fn mood_after_sleep_time_of_day_and_weekday() {
        let v = sample();
        let (from, to) = range();
        let p = patterns(&v, &HashMap::new(), from, to);
        let sleep: Vec<(&str, f32, u32)> = p
            .mood_by_sleep
            .iter()
            .map(|g| (g.key, g.average, g.count))
            .collect();
        assert_eq!(
            sleep,
            vec![("poor", 1.5, 1), ("fair", 3.0, 1), ("good", 4.5, 1)]
        );
        let tod: Vec<(&str, u32)> = p
            .mood_by_time_of_day
            .iter()
            .map(|g| (g.key, g.count))
            .collect();
        assert_eq!(
            tod,
            vec![
                ("night", 1),
                ("morning", 2),
                ("afternoon", 1),
                ("evening", 2)
            ]
        );
        // 2026-10-06 is a Tuesday.
        assert_eq!(p.mood_by_weekday[0].key, "tuesday");
        assert_eq!(p.mood_by_weekday[0].average, 1.5);
    }

    #[test]
    fn top_tags_by_count() {
        let mut v = sample();
        let work = Id::from_bytes([1; 16]);
        let sleep_tag = Id::from_bytes([2; 16]);
        v[0].tags = vec![work];
        v[1].tags = vec![work, sleep_tag];
        let tags = HashMap::from([
            (
                work,
                Tag {
                    id: work,
                    name: "praca".into(),
                    color: "#000000".into(),
                    updated_at: None,
                },
            ),
            (
                sleep_tag,
                Tag {
                    id: sleep_tag,
                    name: "sen".into(),
                    color: "#000000".into(),
                    updated_at: None,
                },
            ),
        ]);
        let (from, to) = range();
        let p = patterns(&v, &tags, from, to);
        let top: Vec<(&str, u32)> = p
            .top_tags
            .iter()
            .map(|c| (c.label.as_str(), c.count))
            .collect();
        assert_eq!(top, vec![("praca", 2), ("sen", 1)]);
    }
}

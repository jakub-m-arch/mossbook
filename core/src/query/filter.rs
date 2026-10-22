//! Timeline filters (E-13): type, tags, date range and full-text search.

use core::fmt;
use std::collections::HashMap;

use serde::Deserialize;

use crate::domain::{Entry, EntryKindTag, Id, Tag, Timestamp};

use super::search::SearchQuery;

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct EntryFilter {
    /// Entry types to include; empty means all.
    pub kinds: Vec<EntryKindTag>,
    /// Entries carrying any of these tags; empty means no tag filter.
    pub tags: Vec<Id>,
    /// Inclusive lower bound of `occurred_at`.
    pub from: Option<Timestamp>,
    /// Exclusive upper bound of `occurred_at`.
    pub to: Option<Timestamp>,
    /// Full-text query; also matches tag names.
    pub text: Option<String>,
    /// Only quick entries still to be completed.
    pub incomplete_only: bool,
}

impl fmt::Debug for EntryFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryFilter")
            .field("kinds", &self.kinds)
            .field("tags", &self.tags.len())
            .field("has_text", &self.text.is_some())
            .finish_non_exhaustive()
    }
}

impl EntryFilter {
    /// Matching entries, newest `occurred_at` first.
    pub fn apply<'a>(
        &self,
        entries: impl IntoIterator<Item = &'a Entry>,
        tags: &HashMap<Id, Tag>,
    ) -> Vec<&'a Entry> {
        let query = self.text.as_deref().and_then(SearchQuery::parse);
        let mut out: Vec<&Entry> = entries
            .into_iter()
            .filter(|e| self.kinds.is_empty() || self.kinds.contains(&e.kind.tag()))
            .filter(|e| self.tags.is_empty() || e.tags.iter().any(|t| self.tags.contains(t)))
            .filter(|e| self.from.is_none_or(|from| e.occurred_at >= from))
            .filter(|e| self.to.is_none_or(|to| e.occurred_at < to))
            .filter(|e| !self.incomplete_only || e.incomplete)
            .filter(|e| {
                query.as_ref().is_none_or(|q| {
                    let tag_names = e
                        .tags
                        .iter()
                        .filter_map(|id| tags.get(id))
                        .map(|t| t.name.as_str());
                    q.matches(e.texts().chain(tag_names))
                })
            })
            .collect();
        out.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at).then(b.id.cmp(&a.id)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EntryKind;
    use time::Duration;
    use time::macros::datetime;

    fn entry(kind: EntryKindTag, hours_ago: i64, note: &str) -> Entry {
        let now = datetime!(2026-10-19 12:00 UTC);
        let mut e = Entry::new(EntryKind::empty(kind), now).unwrap();
        e.occurred_at = (now - Duration::hours(hours_ago)).into();
        e.note = note.into();
        e
    }

    fn tag(id: Id, name: &str) -> Tag {
        Tag {
            id,
            name: name.into(),
            color: "#000000".into(),
            updated_at: None,
        }
    }

    #[test]
    fn filters_combine() {
        let work = Id::from_bytes([1; 16]);
        let mut a = entry(EntryKindTag::Mood, 1, "spokojny poranek");
        a.tags = vec![work];
        let b = entry(EntryKindTag::Fear, 30, "tłum w tramwaju");
        let mut c = entry(EntryKindTag::Fear, 2, "zebranie");
        c.tags = vec![work];
        c.incomplete = true;
        let entries = [a.clone(), b.clone(), c.clone()];
        let tags = HashMap::from([(work, tag(work, "Praca"))]);
        let ids = |f: &EntryFilter| {
            f.apply(&entries, &tags)
                .iter()
                .map(|e| e.id)
                .collect::<Vec<_>>()
        };

        assert_eq!(ids(&EntryFilter::default()), vec![a.id, c.id, b.id]);
        assert_eq!(
            ids(&EntryFilter {
                kinds: vec![EntryKindTag::Fear],
                ..Default::default()
            }),
            vec![c.id, b.id]
        );
        assert_eq!(
            ids(&EntryFilter {
                tags: vec![work],
                ..Default::default()
            }),
            vec![a.id, c.id]
        );
        assert_eq!(
            ids(&EntryFilter {
                incomplete_only: true,
                ..Default::default()
            }),
            vec![c.id]
        );
        assert_eq!(
            ids(&EntryFilter {
                text: Some("TRAMWAJ".into()),
                ..Default::default()
            }),
            vec![b.id]
        );
        // Tag names are searchable too.
        assert_eq!(
            ids(&EntryFilter {
                text: Some("praca".into()),
                ..Default::default()
            }),
            vec![a.id, c.id]
        );
        // Range: from inclusive, to exclusive.
        let range = EntryFilter {
            from: Some(b.occurred_at),
            to: Some(c.occurred_at),
            ..Default::default()
        };
        assert_eq!(ids(&range), vec![b.id]);
        let none = EntryFilter {
            text: Some("   ".into()),
            kinds: vec![EntryKindTag::Sleep],
            ..Default::default()
        };
        assert!(ids(&none).is_empty());
    }

    #[test]
    fn deserializes_from_frontend_json_shape() {
        let f: EntryFilter =
            serde_json::from_str(r#"{ "kinds": ["fear"], "incomplete_only": true }"#).unwrap();
        assert_eq!(f.kinds, vec![EntryKindTag::Fear]);
        assert!(f.incomplete_only && f.tags.is_empty() && f.text.is_none());
        let with_range: EntryFilter =
            serde_json::from_str(r#"{ "from": "2026-10-01T00:00:00+02:00", "tags": [] }"#).unwrap();
        assert!(with_range.from.is_some() && with_range.to.is_none());
    }

    #[test]
    fn debug_hides_query_text() {
        let f = EntryFilter {
            text: Some("very private".into()),
            ..Default::default()
        };
        assert!(!format!("{f:?}").contains("private"));
    }
}

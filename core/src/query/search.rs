//! Full-text search (E-13), case- and diacritic-insensitive.

use core::fmt;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
use zeroize::Zeroizing;

/// Folds text for matching: lowercase, no diacritics (`Zażółć` → `zazolc`).
///
/// NFKD strips most accents; letters that do not decompose (Polish `ł`,
/// Scandinavian `ø`, …) are mapped by hand.
pub fn fold(text: &str) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(text.len()));
    for c in text
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
    {
        match c {
            'ł' => out.push('l'),
            'đ' => out.push('d'),
            'ø' => out.push('o'),
            'ß' => out.push_str("ss"),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            other => out.push(other),
        }
    }
    out
}

/// A parsed search: every whitespace-separated term must occur somewhere.
pub struct SearchQuery {
    terms: Vec<Zeroizing<String>>,
}

impl SearchQuery {
    /// `None` for an empty or whitespace-only query.
    pub fn parse(query: &str) -> Option<Self> {
        let folded = fold(query);
        let terms: Vec<_> = folded
            .split_whitespace()
            .map(|t| Zeroizing::new(t.to_owned()))
            .collect();
        (!terms.is_empty()).then_some(Self { terms })
    }

    /// Whether all terms occur in the given texts (each term may be in a different text).
    pub fn matches<'a>(&self, texts: impl IntoIterator<Item = &'a str>) -> bool {
        let haystack: Vec<Zeroizing<String>> = texts.into_iter().map(fold).collect();
        self.terms
            .iter()
            .all(|term| haystack.iter().any(|h| h.contains(term.as_str())))
    }
}

impl fmt::Debug for SearchQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchQuery")
            .field("terms", &self.terms.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_diacritics() {
        assert_eq!(fold("Zażółć GĘŚLĄ jaźń").as_str(), "zazolc gesla jazn");
        assert_eq!(fold("Ćma ŁÓDŹ").as_str(), "cma lodz");
        // Ukrainian: ї decomposes to і + diaeresis; ґ is a letter of its own and stays.
        assert_eq!(fold("Їжак Ґанок").as_str(), "іжак ґанок");
        assert_eq!(fold("Straße Ærø").as_str(), "strasse aero");
    }

    #[test]
    fn all_terms_must_match_somewhere() {
        let q = SearchQuery::parse("  łódź   TRAMWAJ ").unwrap();
        // Terms match inside words and across different fields.
        assert!(q.matches(["Jazda tramwajem", "w Łodzi"]));
        assert!(q.matches(["Jazda tramwajem po Łódź-Kaliskiej"]));
        assert!(q.matches(["tramwaj", "lodz"]));
        assert!(!q.matches(["tramwaj"]));
    }

    #[test]
    fn empty_query_is_none() {
        assert!(SearchQuery::parse("").is_none());
        assert!(SearchQuery::parse(" \n\t").is_none());
    }

    #[test]
    fn debug_hides_terms() {
        let dbg = format!("{:?}", SearchQuery::parse("secret words").unwrap());
        assert!(!dbg.contains("secret"));
    }
}

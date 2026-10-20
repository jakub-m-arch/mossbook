//! Report for a therapist (X-3): a PDF built from a document model.
//!
//! The UI composes the [`ReportDoc`] from the entries the user selected, in the
//! user's language, and shows the same model as a preview, so preview and PDF
//! match. This module only lays it out on A4 pages; it adds no content of its own.
//! The resulting PDF is **not encrypted** and the UI says so before saving.

mod layout;

use core::fmt;

use serde::Deserialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use layout::render_pdf;

/// Upper bounds that keep a hostile or accidental model from exhausting memory.
pub const MAX_ITEMS: usize = 20_000;
pub const MAX_TEXT_CHARS: usize = 20_000;

/// Minimum length of a password protecting the PDF report. The PDF security
/// handler uses a password-based KDF that is much weaker than the vault's
/// Argon2id, so short passwords are refused rather than giving a false sense of
/// protection.
pub const MIN_REPORT_PASSWORD_CHARS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportError {
    /// The model exceeds [`MAX_ITEMS`] or [`MAX_TEXT_CHARS`].
    TooLarge,
    /// The embedded font could not be loaded (a build problem, not user input).
    Font,
    /// The password is empty or shorter than [`MIN_REPORT_PASSWORD_CHARS`].
    WeakPassword,
    /// The password contains characters that PDF 2.0 passwords cannot hold
    /// (SASLprep rejects e.g. emoji, control characters and private-use code points).
    UnsupportedPassword,
    /// The PDF could not be encrypted (a bug or a library problem).
    Encrypt,
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "report is too large",
            Self::Font => "report font unavailable",
            Self::WeakPassword => "report password is too short",
            Self::UnsupportedPassword => "report password has unsupported characters",
            Self::Encrypt => "report encryption failed",
        })
    }
}

impl std::error::Error for ReportError {}

/// A whole report. All strings are final, translated text.
#[derive(Clone, Default, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct ReportDoc {
    pub title: String,
    /// E.g. the period covered.
    #[serde(default)]
    pub subtitle: String,
    /// Short lines under the title (counts, notes for the reader).
    #[serde(default)]
    pub summary: Vec<String>,
    #[serde(default)]
    pub sections: Vec<ReportSection>,
    /// Printed at the bottom of every page, e.g. "Mossbook · not encrypted".
    #[serde(default)]
    pub footer: String,
    /// Page label pattern with `{page}` and `{pages}`, e.g. "Page {page} of {pages}".
    #[serde(default)]
    pub page_label: String,
}

#[derive(Clone, Default, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct ReportSection {
    /// E.g. a day.
    pub heading: String,
    #[serde(default)]
    pub items: Vec<ReportItem>,
}

#[derive(Clone, Default, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct ReportItem {
    /// E.g. "21:30 · Fear / trigger".
    pub title: String,
    /// E.g. "Intensity 7/10 · tags: work".
    #[serde(default)]
    pub meta: String,
    #[serde(default)]
    pub fields: Vec<ReportField>,
}

#[derive(Clone, Default, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct ReportField {
    pub label: String,
    pub value: String,
}

impl ReportDoc {
    /// Rejects models beyond the size limits.
    pub fn validate(&self) -> Result<(), ReportError> {
        let items: usize = self.sections.iter().map(|s| s.items.len()).sum();
        let too_long = |s: &str| s.chars().count() > MAX_TEXT_CHARS;
        let any_too_long = too_long(&self.title)
            || too_long(&self.subtitle)
            || too_long(&self.footer)
            || self.summary.iter().any(|s| too_long(s))
            || self.sections.iter().any(|s| {
                too_long(&s.heading)
                    || s.items.iter().any(|i| {
                        too_long(&i.title)
                            || too_long(&i.meta)
                            || i.fields
                                .iter()
                                .any(|f| too_long(&f.label) || too_long(&f.value))
                    })
            });
        if items > MAX_ITEMS || any_too_long {
            return Err(ReportError::TooLarge);
        }
        Ok(())
    }
}

impl fmt::Debug for ReportDoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportDoc")
            .field("sections", &self.sections.len())
            .finish_non_exhaustive()
    }
}

//! A4 layout of a [`ReportDoc`]: word wrapping by glyph widths, pagination and a
//! footer with page numbers on every page.

use std::collections::BTreeMap;
use std::sync::Arc;

use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use lopdf::{Document, EncryptionState, EncryptionVersion, Permissions};
use printpdf::{
    Color, FontId, Line, LinePoint, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Pt, Rgb, TextItem,
};
use zeroize::Zeroizing;

use super::{MIN_REPORT_PASSWORD_CHARS, ReportDoc, ReportError};

const REGULAR_TTF: &[u8] = include_bytes!("../../data/fonts/NotoSans-Regular.ttf");
const BOLD_TTF: &[u8] = include_bytes!("../../data/fonts/NotoSans-Bold.ttf");

const PAGE_W: f32 = 595.28; // A4 in points
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 52.0;
const FOOTER_SPACE: f32 = 36.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;
const LINE_FACTOR: f32 = 1.38;

const INK: (f32, f32, f32) = (0.11, 0.13, 0.11);
const MUTED: (f32, f32, f32) = (0.40, 0.42, 0.38);
const RULE: (f32, f32, f32) = (0.85, 0.86, 0.83);

/// A loaded font with what wrapping needs.
struct Face {
    id: FontId,
    parsed: ParsedFont,
}

impl Face {
    /// Width of `text` in points at `size`.
    fn width(&self, text: &str, size: f32) -> f32 {
        let units = f32::from(self.parsed.units_per_em.max(1));
        let advance: u32 = text
            .chars()
            .map(|c| {
                self.parsed
                    .lookup_glyph_index(c as u32)
                    .and_then(|g| self.parsed.get_glyph_width(g))
                    .map_or(0, u32::from)
            })
            .sum();
        advance as f32 * size / units
    }
}

/// Breaks `text` into lines no wider than `max` (by `measure`). Explicit newlines
/// are kept; words longer than a line are split by character.
pub(crate) fn wrap(text: &str, max: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if measure(&candidate) <= max {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // The word alone may still be too wide: split it by character.
            for c in word.chars() {
                let next = format!("{line}{c}");
                if !line.is_empty() && measure(&next) > max {
                    lines.push(std::mem::take(&mut line));
                    line.push(c);
                } else {
                    line = next;
                }
            }
        }
        lines.push(line);
    }
    lines
}

struct Pages {
    done: Vec<Vec<Op>>,
    ops: Vec<Op>,
    /// Distance from the top edge to the next baseline.
    y: f32,
}

impl Pages {
    fn new() -> Self {
        Self {
            done: Vec::new(),
            ops: Vec::new(),
            y: MARGIN,
        }
    }

    /// Starts a new page if `height` does not fit on this one.
    fn ensure(&mut self, height: f32) {
        if self.y + height > PAGE_H - MARGIN - FOOTER_SPACE {
            self.done.push(std::mem::take(&mut self.ops));
            self.y = MARGIN;
        }
    }

    fn text(&mut self, face: &Face, size: f32, color: (f32, f32, f32), x: f32, line: &str) {
        let baseline = PAGE_H - self.y - size;
        self.ops.extend([
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(face.id.clone()),
                size: Pt(size),
            },
            Op::SetFillColor { col: rgb(color) },
            Op::SetTextCursor {
                pos: Point {
                    x: Pt(x),
                    y: Pt(baseline),
                },
            },
            Op::ShowText {
                items: vec![TextItem::Text(line.to_owned())],
            },
            Op::EndTextSection,
        ]);
    }

    /// Wrapped paragraph; each line is kept on one page.
    fn paragraph(
        &mut self,
        face: &Face,
        size: f32,
        color: (f32, f32, f32),
        indent: f32,
        text: &str,
    ) {
        let lh = size * LINE_FACTOR;
        let lines = Zeroizing::new(wrap(text, CONTENT_W - indent, &|s| face.width(s, size)));
        for line in lines.iter() {
            self.ensure(lh);
            self.text(face, size, color, MARGIN + indent, line);
            self.y += lh;
        }
    }

    fn rule(&mut self) {
        let y = PAGE_H - self.y;
        self.ops.extend([
            Op::SetOutlineColor { col: rgb(RULE) },
            Op::SetOutlineThickness { pt: Pt(0.6) },
            Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(MARGIN),
                                y: Pt(y),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(PAGE_W - MARGIN),
                                y: Pt(y),
                            },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            },
        ]);
    }

    fn gap(&mut self, pt: f32) {
        self.y += pt;
    }

    fn finish(mut self) -> Vec<Vec<Op>> {
        self.done.push(self.ops);
        self.done
    }
}

fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb {
        r,
        g,
        b,
        icc_profile: None,
    })
}

fn load(doc: &mut PdfDocument, bytes: &[u8]) -> Result<Face, ReportError> {
    let parsed = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).ok_or(ReportError::Font)?;
    let id = doc.add_font(&parsed);
    Ok(Face { id, parsed })
}

/// Lays out the report on A4 pages and returns the PDF bytes. With a password,
/// the file is encrypted with AES-256 (PDF 2.0, revision 6).
pub fn render_pdf(report: &ReportDoc, password: Option<&str>) -> Result<Vec<u8>, ReportError> {
    report.validate()?;
    let mut doc = PdfDocument::new(&report.title);
    let regular = load(&mut doc, REGULAR_TTF)?;
    let bold = load(&mut doc, BOLD_TTF)?;

    let mut p = Pages::new();
    p.paragraph(&bold, 20.0, INK, 0.0, &report.title);
    if !report.subtitle.is_empty() {
        p.paragraph(&regular, 11.0, MUTED, 0.0, &report.subtitle);
    }
    p.gap(6.0);
    for line in &report.summary {
        p.paragraph(&regular, 10.0, INK, 0.0, line);
    }

    for section in &report.sections {
        p.gap(14.0);
        // Keep a heading together with the start of its first item.
        p.ensure(13.0 * LINE_FACTOR + 40.0);
        p.paragraph(&bold, 13.0, INK, 0.0, &section.heading);
        p.gap(2.0);
        for item in &section.items {
            p.ensure(30.0);
            p.gap(4.0);
            p.rule();
            p.gap(6.0);
            p.paragraph(&bold, 10.5, INK, 0.0, &item.title);
            if !item.meta.is_empty() {
                p.paragraph(&regular, 9.0, MUTED, 0.0, &item.meta);
            }
            for field in &item.fields {
                p.gap(2.0);
                p.paragraph(&bold, 8.5, MUTED, 0.0, &field.label);
                p.paragraph(&regular, 10.0, INK, 0.0, &field.value);
            }
        }
    }

    let pages = p.finish();
    let total = pages.len();
    let pdf_pages = pages
        .into_iter()
        .enumerate()
        .map(|(i, mut ops)| {
            let label = report
                .page_label
                .replace("{page}", &(i + 1).to_string())
                .replace("{pages}", &total.to_string());
            let footer = [report.footer.as_str(), label.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("  ·  ");
            let mut f = Pages {
                done: Vec::new(),
                ops: Vec::new(),
                y: PAGE_H - MARGIN + 8.0,
            };
            f.text(&regular, 8.0, MUTED, MARGIN, &footer);
            ops.extend(f.ops);
            PdfPage::new(Mm(210.0), Mm(297.0), ops)
        })
        .collect();

    let pdf = doc
        .with_pages(pdf_pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    match password {
        Some(password) => encrypt_pdf(pdf, password),
        None => Ok(pdf),
    }
}

/// Encrypts a finished PDF with AES-256 (V5/R6, PDF 2.0). Only the user password
/// is set; permissions stay permissive on purpose, because an owner password does
/// not stop a determined reader from copying or printing the file.
fn encrypt_pdf(pdf: Vec<u8>, password: &str) -> Result<Vec<u8>, ReportError> {
    if password.chars().count() < MIN_REPORT_PASSWORD_CHARS {
        return Err(ReportError::WeakPassword);
    }
    // The rendered PDF is plaintext until the objects are encrypted; wipe the
    // buffer as soon as the parser has copied what it needs.
    let pdf = Zeroizing::new(pdf);
    let mut doc = Document::load_mem(&pdf).map_err(|_| ReportError::Encrypt)?;
    drop(pdf);
    // V5 belongs to PDF 2.0; printpdf writes 1.3 by default.
    doc.version = "2.0".to_owned();

    let mut key = Zeroizing::new([0u8; 32]);
    getrandom::fill(key.as_mut()).map_err(|_| ReportError::Encrypt)?;
    let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
    let state = EncryptionState::try_from(EncryptionVersion::V5 {
        // The /Info dictionary is metadata too; encrypt it as well.
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
        file_encryption_key: &key[..],
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: password,
        user_password: password,
        permissions: Permissions::all(),
    })
    .map_err(|_| ReportError::Encrypt)?;
    doc.encrypt(&state).map_err(|_| ReportError::Encrypt)?;

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|_| ReportError::Encrypt)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{ReportField, ReportItem, ReportSection};

    /// Every character is 10pt wide.
    fn fixed(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    #[test]
    fn wraps_words_newlines_and_long_words() {
        assert_eq!(wrap("aa bb cc", 50.0, &fixed), vec!["aa bb", "cc"]);
        assert_eq!(wrap("one\ntwo", 100.0, &fixed), vec!["one", "two"]);
        assert_eq!(wrap("abcdefghij", 40.0, &fixed), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("", 40.0, &fixed), vec![""]);
        assert_eq!(wrap("x  y", 100.0, &fixed), vec!["x y"]);
    }

    fn sample(items: usize) -> ReportDoc {
        let item = |i: usize| ReportItem {
            title: format!("21:30 · Lęk / wyzwalacz {i}"),
            meta: "Intensywność 7/10 · tagi: praca".into(),
            fields: vec![
                ReportField {
                    label: "Co to wywołało".into(),
                    value: "Tłum w tramwaju, zażółć gęślą jaźń.".into(),
                },
                ReportField {
                    label: "Нотатка".into(),
                    value: "Їжак ґанок — синтетичний текст. ".repeat(12),
                },
            ],
        };
        ReportDoc {
            title: "Mossbook — zapiski".into(),
            subtitle: "1–19 października 2026".into(),
            summary: vec!["Wpisy: 3".into()],
            sections: vec![ReportSection {
                heading: "Poniedziałek, 19 października".into(),
                items: (0..items).map(item).collect(),
            }],
            footer: "Mossbook · ten plik nie jest zaszyfrowany".into(),
            page_label: "Strona {page} z {pages}".into(),
        }
    }

    #[test]
    fn renders_a_pdf_with_polish_and_ukrainian_text() {
        let pdf = render_pdf(&sample(3), None).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        // Fonts are embedded whole (subsetting needs printpdf's heavy text_layout
        // feature); two compressed Noto Sans faces are about 600 KB.
        assert!(pdf.len() < 1_000_000, "{} bytes", pdf.len());
    }

    const PASSWORD: &str = "correct horse battery staple";

    fn page_text(doc: &Document) -> String {
        let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
        doc.extract_text(&pages).unwrap_or_default()
    }

    #[test]
    fn encrypted_pdf_needs_the_password() {
        let pdf = render_pdf(&sample(3), Some(PASSWORD)).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));

        let mut doc = Document::load_mem(&pdf).unwrap();
        assert!(doc.is_encrypted());
        assert!(doc.decrypt("this is not the right password").is_err());

        let plain = Document::load_mem(&render_pdf(&sample(3), None).unwrap()).unwrap();
        let opened =
            Document::load_mem_with_options(&pdf, lopdf::LoadOptions::with_password(PASSWORD))
                .unwrap();
        assert_eq!(page_text(&opened), page_text(&plain));
    }

    #[test]
    fn encrypted_pdf_hides_the_title_metadata() {
        let mut doc = sample(1);
        doc.title = "SECRET-TITLE-123".into();
        let pdf = render_pdf(&doc, Some(PASSWORD)).unwrap();
        let needle = b"SECRET-TITLE-123";
        assert!(!pdf.windows(needle.len()).any(|w| w == needle));
        // Metadata strings in PDF may also be UTF-16BE.
        let utf16: Vec<u8> = "SECRET-TITLE-123"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        assert!(!pdf.windows(utf16.len()).any(|w| w == utf16));
    }

    #[test]
    fn short_or_empty_passwords_are_rejected() {
        assert_eq!(
            render_pdf(&sample(1), Some("too short")).unwrap_err(),
            ReportError::WeakPassword
        );
        assert_eq!(
            render_pdf(&sample(1), Some("")).unwrap_err(),
            ReportError::WeakPassword
        );
    }

    #[test]
    fn unicode_passwords_work() {
        let password = "zażółć-gęślą-jaźń-123";
        let pdf = render_pdf(&sample(1), Some(password)).unwrap();
        let opened =
            Document::load_mem_with_options(&pdf, lopdf::LoadOptions::with_password(password))
                .unwrap();
        assert_eq!(
            page_text(&opened),
            page_text(&Document::load_mem(&render_pdf(&sample(1), None).unwrap()).unwrap())
        );
    }

    #[test]
    fn long_reports_span_several_pages() {
        let short = render_pdf(&sample(1), None).unwrap();
        let long = render_pdf(&sample(60), None).unwrap();
        assert!(long.len() > short.len());
    }

    #[test]
    fn oversized_models_are_rejected() {
        let mut doc = sample(1);
        doc.title = "x".repeat(crate::report::MAX_TEXT_CHARS + 1);
        assert_eq!(render_pdf(&doc, None).unwrap_err(), ReportError::TooLarge);
        let mut doc = sample(0);
        doc.sections[0].items = vec![ReportItem::default(); crate::report::MAX_ITEMS + 1];
        assert_eq!(doc.validate(), Err(ReportError::TooLarge));
    }

    /// Writes a sample report for visual checks: `cargo test -p mossbook-core -- --ignored sample_pdf`.
    #[test]
    #[ignore = "writes target/sample-report.pdf"]
    fn sample_pdf() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/sample-report.pdf");
        std::fs::write(path, render_pdf(&sample(14), None).unwrap()).unwrap();
    }

    /// Writes an encrypted sample for reader checks:
    /// `cargo test -p mossbook-core -- --ignored sample_pdf_encrypted`.
    #[test]
    #[ignore = "writes target/sample-report-encrypted.pdf"]
    fn sample_pdf_encrypted() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/sample-report-encrypted.pdf"
        );
        std::fs::write(
            path,
            render_pdf(&sample(14), Some("correct horse battery staple")).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn debug_hides_content() {
        assert!(!format!("{:?}", sample(1)).contains("tramwaju"));
    }
}

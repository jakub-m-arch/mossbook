# Changelog

All notable changes to Mossbook. Versions follow [Semantic Versioning](https://semver.org).

## 0.1.1 (2026-10-20)

### Fixed
- macOS: the app bundle is now signed (ad-hoc) as a whole. Downloaded 0.1.0 builds
  were only linker-signed, so macOS reported them as damaged and offered to move
  them to the Bin.

### Added
- Settings → About: version, build date and "Check for updates", which opens the
  releases page in the browser (the app itself still never connects).
- Homebrew: `brew install --cask jakub-m-arch/mossbook/mossbook`.

## 0.1.0 (2026-10-20)

First desktop release (Linux, Windows, macOS).

### Journal
- Entry types: mood (1–5 with weather pictograms, energy, emotions), event,
  fear/trigger (symptoms, avoidance, strategy, intensity before and after), sleep,
  nightmare with a rescripted version and rehearsal dates (IRT), and free note.
- "Difficult content": an entry can stay hidden on the timeline until tapped;
  nightmares are hidden by default.
- Quick entry in at most two taps, then straight to the entry's form.
- Quick entry from the lock screen, without the password: it can add an entry but
  read nothing; the entry appears after the next unlock.
- Timeline grouped by day; filters by type, tag, date range and "to complete";
  search that ignores case and diacritics.
- Tags: create, rename, recolor, delete, merge.

### Hard moments
- Safety plan after the Stanley–Brown model, with call buttons for contacts.
- Crisis numbers (Poland: 112, 116 123, 800 70 2222, 116 111) and grounding exercises
  (4-7-8 and box breathing, 5-4-3-2-1), available **without unlocking**.

### Patterns
- Mood over time for a week, a month or three months, with a table view.
- Calendar tinted by each day's average mood; tap a day to see its entries.
- Overviews from 14 entries in a period: most used tags and triggers, mood by the
  previous night's sleep, time of day and weekday, and how much fear intensity went
  down after each strategy. Counts and averages only, no interpretation.

### Report for a therapist
- PDF report for a chosen period, entry types and tags, with a preview that matches
  the file and a clear warning that the PDF is not encrypted.

### Privacy and security
- Everything encrypted at rest (XChaCha20-Poly1305); key from the password via
  Argon2id; 24-word recovery key confirmed at setup.
- No network access of any kind, no telemetry.
- Auto-lock on minimize or after inactivity; lock on exit; privacy curtain when the
  window loses focus; delay after wrong passwords.
- Encrypted `.mossbook` backups: save, restore on a new device, merge or replace.
- Change password, delete all data.
- Clipboard cleared 60 s after copying from the app.

### Languages
- Polish, English and Ukrainian; follows the system language or a chosen one.

# Changelog

All notable changes to Mossbook. Versions follow [Semantic Versioning](https://semver.org).

## 0.1.0 (unreleased)

First desktop release (Linux, Windows, macOS).

### Journal
- Entry types: mood (1–5 with weather pictograms, energy, emotions), event,
  fear/trigger (symptoms, avoidance, strategy, intensity before and after), sleep
  and free note.
- Quick entry in at most two taps, then straight to the entry's form.
- Timeline grouped by day; filters by type, tag, date range and "to complete";
  search that ignores case and diacritics.
- Tags: create, rename, recolor, delete, merge.

### Hard moments
- Safety plan after the Stanley–Brown model, with call buttons for contacts.
- Crisis numbers (Poland: 112, 116 123, 800 70 2222, 116 111) and grounding exercises
  (4-7-8 and box breathing, 5-4-3-2-1), available **without unlocking**.

### Patterns
- Mood over time for a week, a month or three months, with a table view.

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

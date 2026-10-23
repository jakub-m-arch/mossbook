# Changelog

All notable changes to Mossbook. Versions follow [Semantic Versioning](https://semver.org).

## 0.5.0 (2026-10-23)

### Added
- Voice notes can be turned into text, on your device. Off by default: in
  Settings → Turning recordings into text, open the download page, let your browser
  download the speech recognition model (487 MB) and pick the file; Mossbook checks
  that it is exactly that file and unpacks it (about 670 MB). Then a voice note in
  an entry has a "Turn into text" button. The text is shown for you to read and
  correct, is saved with the entry and found by search. Mossbook still never
  connects to the network; recordings are not sent anywhere. Removing the model in
  the settings turns it off. Transcription needs about 1.5 GB of memory; phones
  with less than 6 GB get a warning.

- Press and hold an entry on the timeline (or right-click it on a computer) for its
  menu: open it, hide or stop hiding its content, or delete it.

### Changed
- "Difficult content" is now called "Hiding content" and is on by default for new
  entries written in the form; quick entries stay visible. A report for a therapist
  no longer switches its password on by itself because of hidden entries.
- Confirmations ("Delete this entry?" and the like) open in a window instead of
  appearing under the button, and start on Cancel.
- Delete sits next to Save in an entry.
- On the timeline a hidden entry shows only its type and time, with a small mark;
  tap it to open it. Sleep entries show how long you slept, the quality and the
  awakenings.
- Text fields in the forms are smaller and grow as you write.
- Settings are grouped: a short list (language and lock, backups and sync, voice
  notes, password and recovery key, about), each opening its own screen, instead
  of one long page.
- The readable copy includes the text of voice notes (export format version 3).
- The app is larger: it now contains the speech recognition engine (sherpa-onnx
  with ONNX Runtime, prebuilt libraries pinned by SHA-256). Building Mossbook
  needs CMake and `tools/fetch-sherpa-onnx.sh`; see the README.

## 0.4.0 (2026-10-22)

### Added
- Voice notes: record up to five minutes in an entry, listen to it, record again
  or remove it. Recordings are made inside the app only, stored encrypted like the
  rest of the journal and decrypted only while you listen. They are part of
  backups. On the timeline an entry shows how long its recording is; the report for
  a therapist mentions it without the audio. The first recording asks for access
  to the microphone.

- The same journal on your phone and your computer: Settings → Sync. Mossbook
  writes an encrypted file into a folder that Syncthing shares between your
  devices and merges the other devices' changes; it still never connects to the
  network itself. The newer change wins, and deletions, the plan and the settings
  travel too. Joining asks for the other journal's password and keeps the
  current journal in the automatic backups first.

### Changed
- The readable copy in the settings is now a ZIP archive: the journal as JSON and
  the voice notes as Ogg audio files.
- The journal file and backups use a new format version. They are converted
  automatically; older versions of Mossbook cannot open them, so update Mossbook
  on every device you restore backups on.

### Fixed
- Android with fingerprint unlock: after going to the background the journal could
  stop locking when you left the app, until Mossbook was closed. The fingerprint
  prompt is now shown only once you are back in the app.

## 0.3.0 (2026-10-21)

### Added
- An entry you are writing is no longer lost when the journal locks, for example
  when a call comes in or you switch apps. It is kept encrypted while you type and,
  after unlocking, the journal offers to continue it or discard it. Leaving the form
  yourself still discards it.
- Settings → About: the licence and no-warranty notice, a reminder that Mossbook is
  not a medical device, and the licence texts of every component the app includes.

### Changed
- Website, README and store texts describe a journal, not a treatment: no condition
  names as the audience and no clinical method names. "Body symptoms" is now "Body
  sensations".

## 0.2.0 (2026-10-21)

### Added
- Android (7 and newer): a signed APK on the release page. It holds no network
  permission, is excluded from system backups and device transfers, blocks
  screenshots and screen recording, and asks the keyboard not to learn from what you
  type. CI checks the permissions and the native library of every build.
- Website: Android download with installation steps, fingerprint unlock and the
  Android privacy measures, shown once a release carries an APK.
- Fingerprint unlock on Android (Settings, after the password). The key stays in the
  phone's secure hardware, opens only after a strong fingerprint check and stops
  working when the phone's fingerprints change; the password always works too.

## 0.1.3 (2026-10-21)

### Added
- Therapist PDF reports can be protected with a password (AES-256, PDF 2.0). The
  password is generated in the report form, is never stored, and must be sent through
  a different channel than the file; reports containing "difficult" entries start
  protected. A warning explains what the password does and does not cover.
- Website: <https://jakub-m-arch.github.io/mossbook/> (Polish and English), with no
  cookies, analytics or third-party requests.

## 0.1.2 (2026-10-20)

### Added
- Automatic backups: when the journal locks, at most once a day, an encrypted backup
  is saved next to it and the last seven are kept. They are replaced after a password
  change, deleted with the journal, and can be turned off in Settings → Backup.
- Settings show when you last saved a backup yourself and when the latest automatic
  one was made, with "Show in folder". The backup picker opens in that folder.
- Check your recovery key: type the 24 words and see whether they open your journal.
- Releases carry build provenance attestations (`gh attestation verify`, see
  `SECURITY.md`); CI actions are pinned to commit SHAs.
- Readable copy: save the whole journal as a JSON file (not encrypted; the app warns
  first). The format is in `docs/backup-format.md`.

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
- Plan for a hard moment, with call buttons for contacts.
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

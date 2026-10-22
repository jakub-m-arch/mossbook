# Mossbook: private well-being journal

> The project specification: vision, requirements, threat model, data model,
> cryptography, architecture and implementation order. Decisions taken after this
> document was written are recorded in [`docs/PLAN.md`](docs/PLAN.md) §3 and bind the
> implementation just like this file; where they changed the specification, the text
> below has been updated and points to the decision (D-n).
>
> The normative description of the implemented formats is in
> [`docs/crypto.md`](docs/crypto.md) and [`docs/backup-format.md`](docs/backup-format.md);
> how the code is organised is in [`docs/architecture.md`](docs/architecture.md).

---

## 1. Vision

Mossbook is an open-source app for keeping a personal journal of well-being, events,
fears, sleep and nightmares. It is meant for everyone, but designed with people living
with PTSD or anxiety disorders, and people working with a therapist, in mind.

**The promise to the user:** nobody but you will read your entries. There is no
server, no account and no telemetry. Data is encrypted on your device, and the code is
open, so anyone can check.

### Non-negotiable principles

1. **Zero network.** The app makes no network connections. On Android the release
   build does not even hold the `INTERNET` permission. (The optional transcription
   model, E-19, is downloaded by the user's browser, never by the app; D-23.)
2. **Everything encrypted at rest.** No entry content, tag, date or setting is stored
   on disk in plaintext.
3. **Zero telemetry and analytics**, "anonymous" included. No error reporting to
   anyone.
4. **The user owns the data.** Full export, full deletion, an open backup format.
5. **Not a medical device.** The app does not diagnose, recommend treatment or assess
   health. It is a tool for writing down and organising one's own observations.
6. **Calm instead of engagement.** No streaks, points, badges or guilt-inducing
   notifications.

---

## 2. Threat model

| Threat | Example | Protection |
|---|---|---|
| A housemate or partner with access to the unlocked device | Someone picks up the phone and opens the app | A separate app password; auto-lock on leaving the app and after inactivity |
| Physical access to the files | Theft, repair shop, disk image | All content encrypted (XChaCha20-Poly1305) with a key derived from the password via Argon2id |
| Automatic cloud backups | iCloud, Google Backup | The app is excluded from system backups; even if they leak, the data is encrypted |
| Screen observation | App switcher, screenshots, screen recording | `FLAG_SECURE` (Android), a curtain when going to the background (iOS) or losing focus (desktop) |
| Notifications | Content visible on the lock screen | Local notifications only, with neutral text ("Time for a moment for yourself") |
| Clipboard | A copied passage stays on the clipboard | Clipboard cleared after 60 s |
| App logs | Entry content ends up in logcat or the console | Logging user data is forbidden, enforced by review and lints |
| Forgotten password | Loss of access | A recovery key generated with the vault; a clear statement that without it the data cannot be recovered |
| Tampering with the file | Swapped or corrupted records | Authenticated encryption (AEAD) with AAD bound to the record id |

**Out of scope:** a device with malware or root access, keyloggers, physical coercion
("show me your phone"), memory analysis while the vault is unlocked.

**Metadata leaks we accept and document:** the number of records and of each record
type, the approximate size of entries, and the modification time of the database file.
The full list is in [`docs/threat-model.md`](docs/threat-model.md).

---

## 3. Users

- **Anna, 34, PTSD after an accident.** Wants to note what threw her off balance and
  how she coped, to show her therapist. Often writes when she feels bad, so an entry
  must take seconds.
- **Marek, 41, nightmares.** Works with Imagery Rehearsal Therapy (IRT). Needs a place
  for the nightmare and its "new version" that he rehearses.
- **Ola, 27, generalised anxiety.** Wants to see whether anxiety relates to sleep, work
  or time of day.
- **A therapist (indirectly).** Receives a printout or PDF for a chosen period from the
  patient. Has no access to the app.

---

## 4. Features

Priorities: **MVP** (first release), **v1** (next), **Later**. Implementation status is
tracked in [`docs/PLAN.md`](docs/PLAN.md).

### 4.1. Vault and access

| ID | Feature | Priority |
|---|---|---|
| V-1 | Creating the vault: a password (min. 8 characters, strength meter) and a generated **recovery key** (24 words) that the user must confirm by retyping 3 random words | MVP |
| V-2 | Unlocking with the password | MVP |
| V-3 | Auto-lock: immediately on going to the background (default) or after 1, 5 or 15 minutes, configurable | MVP |
| V-4 | Manual lock with a single, always visible button | MVP |
| V-5 | Changing the password without re-encrypting the data (key rewrap, see §6) | MVP |
| V-6 | Regaining access with the recovery key and setting a new password | MVP |
| V-7 | Biometric unlock (optional, requires the password first) | v1 |
| V-8 | Delay after wrong attempts (1 s, 2 s, 4 s… max 60 s); data is never wiped automatically | MVP |
| V-9 | Permanently deleting the vault, confirmed with the password | MVP |
| V-10 | A neutral app icon and name to choose from (e.g. "Notes") | Later |

**V-1 acceptance criteria:** the user cannot continue without confirming the recovery
key. The screen says plainly: "If you lose your password and this key, nobody (us
included) can recover your entries."

### 4.2. Entries

Shared structure: every entry has a type, the time it happened (default "now",
editable), an optional mood rating, intensity, tags and a text note.

| ID | Entry type | Specific fields | Priority |
|---|---|---|---|
| E-1 | **Mood** | rating 1–5 (pictograms, not emoji faces), energy 1–5, optional emotions from a list (fear, sadness, anger, shame, relief, calm…) | MVP |
| E-2 | **Event** | what happened, where (text, **no GPS**), body reaction, thoughts, what helped | MVP |
| E-3 | **Fear / trigger** | trigger, intensity 0–10, physical symptoms (list + own), whether there was avoidance, the strategy used, intensity after the strategy 0–10 | MVP |
| E-4 | **Sleep** | bedtime, wake time, quality 1–5, number of awakenings, whether there was a nightmare | MVP |
| E-5 | **Nightmare (IRT)** | the nightmare (optionally hidden by default), the rescripted version, dates the new version was rehearsed | v1 |
| E-6 | **Free note** | text only | MVP |

| ID | Feature | Priority |
|---|---|---|
| E-10 | **Quick entry**: at most 2 taps (type, intensity), the rest can be completed later. The entry is marked "to complete" | MVP |
| E-11 | Editing and deleting entries (deleting asks for confirmation) | MVP |
| E-12 | Tags: create, rename, merge, colour | MVP |
| E-13 | Timeline with filters: type, tags, date range, full-text search (in memory after unlocking) | MVP |
| E-14 | Calendar view, each day tinted by its average mood | v1 |
| E-15 | **Quick entry without unlocking**: adding an entry without being able to read existing ones (see §6.4) | v1 |
| E-16 | Hiding the content of chosen entries until tapped ("difficult content") | v1 |
| E-17 | An entry form being filled in survives a lock: kept encrypted and offered after unlocking, discarded when the form is left on purpose (D-20) | v1 |
| E-18 | **Voice note**: up to 5 minutes recorded in the app, attached to an entry, encrypted like the rest and included in backups; no video or other files; recorded on every platform; the readable copy becomes a ZIP with the recordings (D-21) | v2 |
| E-19 | **Voice note to text**: off by default; turned on per device by importing a speech recognition model the user downloads in the browser; a recording is transcribed on the device on request, and the text is shown for the user to correct before it is saved with the entry and becomes searchable (D-23) | v2 |

### 4.3. Plan for a hard moment

A plan the user writes in their own words at a calmer time, to have at hand when
things get hard. The sections and their wording are our own (D-19).

| ID | Feature | Priority |
|---|---|---|
| P-1 | Sections: my warning signs; what I can do on my own; places and people that calm me; who I can call; professionals; making my surroundings safe | MVP |
| P-2 | One-tap access to the plan from the main screen | MVP |
| P-3 | Crisis phone numbers (per country; Poland by default, see §9) visible **without unlocking** the vault. This is the only content available without the password, and it is static, never user data | MVP |
| P-4 | Offline grounding exercises: 4-7-8 breathing, box breathing with an animation, the 5-4-3-2-1 technique. Available without unlocking | MVP |

### 4.4. Patterns (local analysis)

| ID | Feature | Priority |
|---|---|---|
| A-1 | Mood over time (week, month, 3 months) | MVP |
| A-2 | Most frequent tags and triggers in a period | v1 |
| A-3 | Overviews: mood versus the previous night's sleep, time of day, day of the week | v1 |
| A-4 | Strategy effectiveness: average drop in fear intensity after a given strategy | v1 |

**Language rules for A-x:** observations only, never clinical conclusions. Good: "On
days after sleep rated 1–2 your mood was lower on average." Bad: "You have a sleep
problem" or "Your condition is getting worse." With little data (< 14 entries in the
period) overviews are not shown.

### 4.5. Export, backups, deletion

| ID | Feature | Priority |
|---|---|---|
| X-1 | **Encrypted backup**: a single `.mossbook` file encrypted with the same key (opens with the password or the recovery key). Saved through the system file dialog | MVP |
| X-2 | Importing a backup: merge (no duplicates, by id) or replace | MVP |
| X-3 | **Report for a therapist (PDF)**: choice of period, types and tags; preview before saving; optional password protection (AES-256, PDF 2.0); a clear warning when the PDF is **not encrypted** and about sending the password through a different channel | v1 |
| X-4 | Plain JSON export (full portability), with a warning; with voice notes (E-18) a ZIP holding the JSON and the recordings (D-21) | v1 |
| X-5 | Deleting all data (V-9) | MVP |
| X-6 | **Automatic local backups** when the journal locks (daily, newest seven), plus a note in the settings of when a backup was last saved elsewhere (D-16) | v1 |
| X-7 | **Checking the recovery key**: type the 24 words and see whether they open this journal, without changing anything | v1 |
| X-8 | **The same journal on the user's devices**: each device writes an encrypted sync file into a folder Syncthing carries between them; Mossbook itself never connects. The newer edit wins, deletions travel, plan and settings too; pairing makes the second device take the first one's journal, password and recovery key (D-22) | v2 |

### 4.6. Reminders

Postponed to the mobile versions (D-14): a reminder must arrive while the journal is
locked, and on desktop that is only possible while the app is running.

| ID | Feature | Priority |
|---|---|---|
| R-1 | Local reminders at chosen times, **off** by default | v1 (mobile) |
| R-2 | Neutral, configurable text, never containing data from entries | v1 (mobile) |
| R-3 | No "catch-up" reminders and no messages about breaks | v1 (mobile) |

### 4.7. Languages and accessibility

- Polish, English and Ukrainian from the MVP; other languages from the community
  (translation files in the repository).
- Dark and light mode following the system, large touch targets (min. 48 px), screen
  reader support, no animation under `prefers-reduced-motion`.

---

## 5. Data model

All structures below exist **only** in memory after unlocking, or as encrypted blobs.
Serialisation: `serde` + CBOR (`ciborium`). Timestamps are RFC 3339 strings with the
local UTC offset. The exact encoding is in [`docs/crypto.md`](docs/crypto.md)
(*Record contents*).

```rust
pub struct Entry {
    pub id: Id,                      // random UUIDv4 (v7 would reveal the creation time, D-13)
    pub schema: u16,                 // record schema version
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub occurred_at: Timestamp,      // when it happened
    pub kind: EntryKind,
    pub mood: Option<u8>,            // 1..=5
    pub intensity: Option<u8>,       // 0..=10
    pub tags: Vec<Id>,
    pub note: String,
    pub incomplete: bool,            // quick entry still to complete
    pub sensitive: bool,             // hide the content until tapped
    pub voice: Option<VoiceNote>,    // E-18: { id, duration_ms, mime, transcript } → a row of `media`
}

pub enum EntryKind {
    Mood      { energy: Option<u8>, emotions: Vec<Emotion> },
    Event     { what: String, place: String, body: String, thoughts: String, helped: String },
    Fear      { trigger: String, symptoms: Vec<Symptom>, avoidance: Option<bool>,
                strategy: Option<String>, intensity_after: Option<u8> },
    Sleep     { bedtime: Option<Timestamp>, wake: Option<Timestamp>,
                quality: Option<u8>, awakenings: Option<u8>, nightmare: bool },
    Nightmare { content: String, rescripted: String, rehearsals: Vec<Timestamp> },
    Note,
}

pub struct Tag { pub id: Id, pub name: String, pub color: String }

pub struct SafetyPlan {
    pub warning_signs: Vec<String>,
    pub self_coping: Vec<String>,
    pub calming_people_places: Vec<String>,
    pub contacts: Vec<Contact>,          // Contact { name, phone, note }
    pub professionals: Vec<Contact>,
    pub safe_environment: Vec<String>,
}

pub struct Settings {
    pub autolock: AutoLock,              // Immediately | After1Min | After5Min | After15Min
    pub locale: Option<String>,          // None = follow the system language
    pub crisis_country: String,
    pub reminders: Vec<Reminder>,
}

pub struct InboxEntry {                  // a quick entry written while locked (§6.4)
    pub kind: EntryKindTag,
    pub value: Option<u8>,
    pub occurred_at: Timestamp,
}
```

Emotions and symptoms are enums with translation keys plus a `Custom(String)` variant.
The UI never sets ids or timestamps: it sends an `EntryDraft` and the core fills them in.

---

## 6. Cryptography

### 6.1. Key hierarchy

```
password ──Argon2id(salt, params)──▶ KEK_pwd ──┐
                                               ├─ wrap ─▶ DEK (random, 256 bit)
recovery key (24 words, 256 bit) ──────────────┘           │
   └─HKDF─▶ KEK_rec                                         ├─▶ encrypts records (XChaCha20-Poly1305)
                                                            └─▶ encrypts the X25519 private key (quick-entry inbox)
```

- The **DEK** (data encryption key) is random and never changes. Changing the password
  only rewraps the DEK under a new KEK; the data is not re-encrypted.
- **Argon2id**: `m = 64 MiB, p = 1`; `t` starts at 3 and is raised at vault creation
  until one derivation takes about 0.75 s on the device (max 16). The parameters are
  stored in the vault header so they can be raised later, and are validated against
  upper bounds before use.
- **Recovery key**: 256 random bits encoded as 24 BIP-39 words (English wordlist).
  `KEK_rec = HKDF-SHA256(entropy, info = "mossbook/recovery/v1")`.
- **Key wrapping**: XChaCha20-Poly1305 with a random nonce,
  `AAD = "mossbook/wrap/v1" || vault_id`.

### 6.2. Record encryption

- Every record: `nonce (24 B, random) || ciphertext || tag (16 B)`.
- `AAD = "mossbook/rec/v1" || vault_id || record_id || record_type`, which binds a blob
  to its place in the database and prevents swapping records.
- Record types: `entry`, `tag`, `plan`, `settings`; recordings (E-18) use `voice` and
  live in the `media` table.

### 6.3. Secrets in memory

- Keys live in `secrecy::SecretBox` / `zeroize::Zeroizing` and are wiped on lock and
  on exit. Decrypted domain objects are zeroed on drop.
- **The frontend never receives a key.** The web view only gets decrypted domain
  objects through Tauri commands.
- A password from the frontend reaches Rust as a `String` that is moved into
  `Zeroizing<String>` at once. The JavaScript limitation (a string in the web view
  cannot be wiped) is documented in [`docs/threat-model.md`](docs/threat-model.md).

### 6.4. Quick entry without unlocking (v1)

- The vault has an X25519 key pair. The public key is stored in plaintext in the
  header; the private key is encrypted under the DEK.
- A quick entry written without the password is encrypted as a **sealed box**
  (`crypto_box`, libsodium-compatible) to the public key and stored in the `inbox`
  table.
- After unlocking, the app opens the inbox, validates each entry, moves it into
  `records` (encrypted under the DEK) and clears `inbox`.
- Result: without the password one can **add** an entry, but cannot **read** anything.

### 6.5. Biometrics (v1, mobile)

- When enabled: the DEK is wrapped under a key from the system's secure storage
  (Android Keystore with `setUserAuthenticationRequired`, iOS Keychain with
  `biometryCurrentSet`), via `tauri-plugin-biometric` or a custom plugin.
  Android: a custom plugin; a random device key wraps the DEK and the Keystore
  encrypts the device key (details in `docs/crypto.md`, decision D-18).
- Changing the enrolled fingerprints or faces invalidates the key and requires the
  password.
- Desktop: no biometrics; password only.

### 6.6. Libraries (Rust, RustCrypto)

| Purpose | Crate |
|---|---|
| AEAD | `chacha20poly1305` (XChaCha20Poly1305) |
| Password KDF | `argon2` |
| Key KDF | `hkdf` + `sha2` |
| Sealed box | `crypto_box` (feature `seal`) |
| Mnemonic | `bip39` |
| Randomness | `rand_core::OsRng` / `getrandom` |
| Secrets | `secrecy`, `zeroize` |

**No home-grown cryptographic primitives.** Only compositions of existing ones.

---

## 7. Storage

The file `vault.db` (SQLite through `rusqlite` with the `bundled` feature) in the
app's data directory.

```sql
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value BLOB NOT NULL
) STRICT;
-- keys: format_version, vault_id, kdf_params, kdf_salt,
--       wrapped_dek_pwd, wrapped_dek_rec, x25519_pub, wrapped_x25519_priv

CREATE TABLE records (
  id     BLOB PRIMARY KEY,   -- UUID (16 B)
  type   INTEGER NOT NULL,   -- 1 entry, 2 tag, 3 plan, 4 settings
  blob   BLOB NOT NULL       -- nonce || ciphertext || tag
) STRICT;

CREATE TABLE inbox (
  id     BLOB PRIMARY KEY,
  blob   BLOB NOT NULL       -- sealed box
) STRICT;

CREATE TABLE media (         -- format 2: voice notes (E-18, D-21)
  id     BLOB PRIMARY KEY,
  type   INTEGER NOT NULL,   -- 5 voice
  blob   BLOB NOT NULL       -- nonce || ciphertext || tag; Ogg Opus inside
) STRICT;
```

- **No plaintext dates or search fields in columns.** After unlocking, all records are
  loaded into memory; filtering and search happen in Rust. At realistic volumes (a few
  thousand entries, a few MB) this is fast and simple. Recordings are the exception:
  they are decrypted one at a time, only for playback.
- `PRAGMA journal_mode = WAL`, `PRAGMA secure_delete = ON`.
- `VACUUM` after bulk deletion.
- SQL schema migrations in `core/migrations/` (tracked with `PRAGMA user_version`);
  record schema migrations through the `schema` field.

**The `.mossbook` backup format:** the magic `"MOSSBOOK"`, a format version, and a
CBOR body holding the header (vault id, KDF parameters, wrapped DEKs) and the encrypted
records copied 1:1, plus a manifest sealed under the DEK. It is described in
[`docs/backup-format.md`](docs/backup-format.md) so that an independent tool can read
it.

---

## 8. Application architecture

The code-level description is in [`docs/architecture.md`](docs/architecture.md); this
section holds the requirements.

### 8.1. Stack

- **Tauri 2**: desktop (Linux, Windows, macOS) and mobile (Android, iOS) from one code
  base.
- **Rust**: all domain logic, cryptography, storage and PDF reports.
- **Svelte 5 + TypeScript + SvelteKit** (static SPA, Vite): presentation only.
- Charts: hand-written SVG, no chart library (D-12).
- PDF: `printpdf`, generated in the core and saved through the system save dialog.

### 8.2. Layers

**Rule:** `core` (`mossbook-core`) does not depend on Tauri and is tested on its own.
`app/src-tauri` is a thin layer: it maps commands to core functions and holds the
session state. The UI talks to Rust only through the typed wrappers in
`app/src/lib/api/`.

### 8.3. State and commands

The session is a state machine `NoVault → Creating → Locked ⇄ Unlocked`. The unlocked
state owns the decrypted journal; dropping it wipes keys and content.

All commands return `Result<T, AppError>`, where `AppError` has a code and a message
without user data. The current list is `app/src-tauri/src/command_names.rs`; a test
keeps it in sync with the handler, the capability file and the TypeScript wrappers.
Main groups:

| Group | Commands |
|---|---|
| Vault | `vault_status`, `vault_create`, `vault_cancel_create`, `vault_confirm_recovery`, `vault_unlock`, `vault_unlock_recovery`, `vault_lock`, `vault_change_password`, `vault_verify_recovery`, `vault_destroy` |
| Journal | `entries_list`, `entry_get`, `entry_save`, `entry_delete`, `quick_entry`, `quick_entry_locked`, `tags_list`, `tag_create`, `tag_save`, `tag_delete`, `tag_merge`, `plan_get`, `plan_save`, `settings_get`, `settings_save` |
| Patterns | `stats_mood`, `stats_patterns` |
| Files | `backup_export`, `backup_pick`, `backup_import`, `backup_info`, `backup_reveal`, `export_json`, `report_pdf` |
| Help | `crisis_lines`, `dial` |
| App | `clock_sync`, `activity_ping`, `clipboard_clear_if`, `app_info`, `open_releases` |

Auto-lock: window and app lifecycle events (going to the background, minimising) lock
the vault on the Rust side. On desktop, losing focus only covers the window (D-11). The
idle timer lives in Rust; the frontend only resets it.

### 8.4. Platform security

| Platform | Measure |
|---|---|
| All | CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost; media-src blob:; img-src 'self' data:; style-src 'self' 'unsafe-inline'` (`blob:` plays a decrypted voice note from memory); no `http` plugin; Tauri capabilities limited to the app's own commands; fonts and assets local only; the web view may ask for the microphone (voice notes) and is refused every other permission |
| Android | `android.permission.INTERNET` removed from the release manifest (`tools:node="remove"`; the debug build needs it for the dev server); `android:allowBackup="false"` + `dataExtractionRules` excluding everything; `FLAG_SECURE` in `MainActivity` (gen/android); `RECORD_AUDIO` asked on the first recording, no camera or location |
| iOS | Data directory excluded from backups (`isExcludedFromBackup`); a curtain on `willResignActive`; file protection `NSFileProtectionComplete` |
| Desktop | Window curtain on focus loss, data in the user's application data directory; macOS: `NSMicrophoneUsageDescription` and the `com.apple.security.device.audio-input` entitlement (the hardened runtime refuses the microphone silently without it) |

Android: the native configuration lives in `app/src-tauri/gen/android`, which is
committed. `tauri android init` regenerates it from a template and would drop these
changes, so the project is edited in place and a test checks the hardening. To verify
for iOS: the same for `gen/apple`.

### 8.5. UI and UX

- **Lock screen**: the password field and two buttons available without the password:
  "Plan for a hard moment / phone numbers" (crisis numbers and exercises only, no user
  data) and "Quick entry".
- **Main screen**: a large quick-entry button, the timeline, the lock button.
- A calm palette, no red for warnings around emotional content, no unnecessary pop-ups.
- Every form can be saved after filling in one field.
- Plain, warm language in the second person, no clinical jargon.

---

## 9. Sensitive content

- **Crisis phone numbers (Poland, re-checked before every release):**
  - 112: emergency number,
  - 116 123: helpline for adults in emotional crisis,
  - 800 70 2222: support centre for adults in mental crisis (24/7),
  - 116 111: helpline for children and young people.
- The numbers are kept in `core/data/crisis/{country}.json` with their sources and the
  date they were checked. Other countries come from the community via pull requests.
- A notice in the settings and on the first screen: the app does not replace
  professional help; in immediate danger call 112.
- The grounding exercise texts and the IRT descriptions should be reviewed by a
  psychotherapist before v1.

---

## 10. Testing and quality

- **Cryptography**: round-trips, wrong password, wrong recovery key, a flipped byte in
  a blob (must fail), swapped records (the AAD must catch it), a password change keeps
  the data, fixed test vectors for the format, cross-checked by independent tools.
- **Property tests** (`proptest`): serialising and encrypting arbitrary `Entry` values.
- **Backups**: export → import into an empty vault → identical data; importing older
  format versions.
- **Migrations**: a database from every earlier version opens correctly (committed
  fixtures).
- **No network**: CI checks that the dependency tree contains no `reqwest`, `hyper`,
  `ureq` etc. (`cargo deny` with banned crates; the one exception is the sherpa-onnx
  build script, which is handed archives checked against pinned SHA-256 sums, D-23);
  a test of the Android release manifest; the release APK's native libraries import no
  socket functions.
- **No leaks to logs**: `AppError` and `Debug` for domain types never print content
  (hand-written, redacting `Debug`).
- **Frontend**: `svelte-check`, unit tests (Vitest), end-to-end scenarios (Playwright):
  creating the vault, an entry, locking, unlocking.
- **Test data** is synthetic only, generated in code.

CI (GitHub Actions): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
`cargo deny check`, `cargo audit`, `npm run check`, `npm run lint`, `npm test`, the
end-to-end tests, desktop builds for three systems, and (with the Android build) an APK.

---

## 11. Distribution and licence

- **Licence**: `GPL-3.0-or-later` with an additional permission (section 7) for
  distribution through app stores, see `LICENSE-EXCEPTION.md`. Copyleft prevents closed
  forks with telemetry added.
- **GitHub Releases**: desktop installers (and later the APK) with `SHA256SUMS`.
- **Homebrew** (macOS): a cask in `jakub-m-arch/homebrew-mossbook`, updated by CI when
  a release is published.
- **Updates** never come from the app itself (principle 1, D-15): the user checks the
  releases page or relies on a package manager.
- **F-Droid**: an app without network or Google dependencies is an ideal candidate.
  Requires a reproducible build.
- Google Play and the App Store maybe later. Mind the "health" category in the stores
  and their data declarations (ours: "we collect no data").

---

## 12. Implementation order

1. **Skeleton**: Cargo workspace (`core`, `app/src-tauri`), Svelte 5 + Vite, CI with
   fmt/clippy/test/deny. An empty app starts on desktop.
2. **Core: crypto**: `kdf`, `aead`, `wrap`, `recovery` with full tests. No UI.
3. **Core: vault and storage**: create/unlock/lock/rekey, SQLite, tables, migrations,
   integration tests.
4. **Core: domain**: `Entry`, `Tag`, `SafetyPlan`, `Settings`, CRUD, filter, search.
5. **Tauri: commands and state**, auto-lock on the Rust side.
6. **UI: vault creation and lock screen** (with the recovery key and its confirmation).
7. **UI: quick entry + full form + timeline.**
8. **UI: plan for a hard moment + phone numbers + exercises** (available from the lock
   screen).
9. **`.mossbook` backup**: export and import.
10. **Mood chart (A-1)**, i18n pl/en/uk.
11. **Android**: build, `INTERNET` removed, `FLAG_SECURE`, backups disabled. Tested on
    a device. *(Postponed.)*
12. **MVP release 0.1.0** (desktop), `SECURITY.md`, `docs/threat-model.md`.
13. **v1**: biometrics, quick entry without unlocking, IRT, PDF report, overviews
    A-2–A-4, reminders, iOS.

Rules for contributors and agents are in [`CLAUDE.md`](CLAUDE.md).

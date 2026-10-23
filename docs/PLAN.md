# Mossbook: implementation plan

The working plan for [`SPEC.md`](../SPEC.md) (§12): stages, their state, and the
decisions taken along the way (§3). Update the checklist after every stage.
Legend: `[ ]` to do, `[x]` done.

---

## 1. Environment (one-off)

As of 2026-10-18 (macOS): Node 26 and npm, Xcode Command Line Tools, Rust 1.99.1,
tauri-cli 2.12.3, cargo-deny 0.20.2, cargo-audit 0.22.2. **Full Xcode, a JDK and the
Android SDK are not installed.**

- [x] Rust: `rustup` (stable) + `rustfmt`, `clippy`
- [x] `cargo install tauri-cli cargo-deny cargo-audit --locked`
- [x] (stage 11) JDK 17, Android SDK command-line tools + NDK (no Android Studio needed), `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android`
- [ ] (v1, iOS) full Xcode

---

## 2. Stages

### Stage 1: Skeleton
- [x] Cargo workspace: `core` (`mossbook-core`), `app/src-tauri` (`mossbook`); `rust-toolchain.toml` (1.99.1), resolver 3, edition 2024, `release` profile (`lto`, `codegen-units = 1`, `panic = "abort"`, `strip`)
- [x] `app/`: SvelteKit 3 + `adapter-static` (SPA, `fallback: index.html`, `ssr = false`), Svelte 5 (runes enforced), TS, Vite 8, Vitest, ESLint, Prettier
- [x] Tauri 2.12: CSP from SPEC §8.4 (+ `devCsp` with the HMR websocket), `freezePrototype`, `tauri-plugin-log` removed; identifier `io.github.jakubmarch.mossbook`
- [x] `deny.toml`: GPL-3.0-compatible licences, networking crates and `tauri-plugin-log` banned, crates.io only
- [x] "No logging" lints: clippy `print_stdout`/`print_stderr`/`dbg_macro` + `disallowed-macros` for `log::*`/`tracing::*`; ESLint `no-console`
- [x] CI: fmt, clippy, test, deny, audit, `npm run check`/`lint`/`test`, e2e, desktop builds on Ubuntu/Windows/macOS (unsigned)
- [x] The empty app starts; CI green on GitHub

**Findings:**
- On Android/iOS Tauri always depends on `reqwest` (+ `hyper`) but uses it only under `cfg(all(dev, mobile))` for the dev-server proxy. `deny.toml` allows `reqwest` only as a dependency of `tauri`. Stage 11: check that the release `.so` contains no `reqwest`/`hyper` symbols, in addition to the manifest test.
- `cargo audit` warns (unmaintained/unsound) about gtk-rs 0.18 on Linux, which comes with Tauri; also zbus via the opener plugin. They do not block CI and are silenced in `deny.toml` with reasons.
- GitHub Actions are pinned to commit SHAs (stage 14).

### Stage 2: Core – cryptography (`core/src/crypto/`)
- [x] `kdf.rs`: Argon2id, `KdfParams { m_kib, t, p }`, 13-byte header encoding, upper-bound validation, `calibrate(CALIBRATION_TARGET)` (raises only `t`), NFC password normalisation
- [x] `aead.rs`: XChaCha20-Poly1305, `nonce(24) || ct || tag(16)`, record AAD from SPEC §6.2, `RecordType` with column codes
- [x] `wrap.rs`: DEK wrap/unwrap, AAD `"mossbook/wrap/v1" || vault_id`
- [x] `recovery.rs`: 256 bit ↔ 24 BIP-39 words, HKDF-SHA256 → KEK_rec, input normalisation, `confirm()` for V-1
- [x] `Dek`, `Kek`, `RecoveryKey` in `SecretBox`, no `Clone`, redacted `Debug`; intermediate buffers zeroed
- [x] Tests: round-trips, wrong password/key, every flipped byte, swapped records, other type/vault, hostile KDF parameters, the whole hierarchy + password change, BIP-39 vector, proptest
- [x] Fixed vectors confirmed independently (libsodium, argon2-cffi): `tools/verify-vectors.py`
- [x] `docs/crypto.md`

### Stage 3: Core – vault and storage (`vault/`, `store/`)
- [x] `store/`: `rusqlite` (`bundled`), schema from SPEC §7 (`STRICT`), WAL, `secure_delete = ON`, migrations via `PRAGMA user_version`, refuses databases from a newer version
- [x] `VaultHeader` in `meta`, `UnlockedVault` (DEK + records in memory, zeroed on drop)
- [x] `PendingVault` (D-3): the vault lives in memory and is written only after the 3 words are confirmed, via `vault.db.creating` + rename
- [x] `unlock`, `unlock_with_recovery` (+ new password), `lock` (WAL checkpoint), `change_password` (rewrap, new salt), `destroy` (hands the vault back on a wrong password)
- [x] `UnlockThrottle`: 1, 2, 4… 60 s, in memory only (D-5)
- [x] Corrupted records are skipped and counted instead of blocking the whole vault
- [x] Integration tests: life cycle, wrong password/phrase, recovery, password change, deletion, no plaintext on disk, tampered/swapped records, tampered header, format v1 fixture
- [x] `docs/crypto.md`: *Vault file*

### Stage 4: Core – domain and queries
- [x] `domain/`: `Entry` (+ six `EntryKind`s), `Tag`, `SafetyPlan`, `Settings`, `Emotion`/`Symptom` with `Custom(String)`; range validation; random UUIDv4 ids (D-13); CBOR; RFC 3339 timestamps with offset; `ZeroizeOnDrop` and redacted `Debug`
- [x] `Journal`: entry CRUD (keeps `created_at`, rejects unknown tags), quick entry, tags (create, rename/recolour, delete and merge in one transaction), plan and settings as singletons
- [x] `UnlockedVault` hands over decrypted records once (`take_loaded`), so content stays in memory only as domain objects; batched writes (`write_batch`)
- [x] `query/`: filter (type, tags, date range, incomplete only), search ignoring case and diacritics (including `ł`, and tag names), `mood_by_day` for A-1
- [x] Tests: domain and query unit tests, journal integration tests, proptest: any `Entry` → save → lock → read
- [x] `docs/crypto.md`: *Record contents*

### Stage 5: Tauri – commands and state
- [x] `Session` (plain Rust, testable): `no_vault` / `creating` / `locked` / `unlocked`, wrong-attempt delay, password change, deletion (a wrong password leaves the journal untouched)
- [x] Commands from SPEC §8.3, all `async` (Argon2 does not block the UI); passwords moved into `Zeroizing` at once; `AppError { code, message, retry_after_ms }`
- [x] Auto-lock in Rust: `Suspended` (mobile) and minimising (desktop) lock under the "immediately" policy; focus loss only emits `privacy-curtain` (D-11); idle timer every 5 s, reset by the frontend (`activity_ping`); system dialogs pause the background lock but not the timer (D-6)
- [x] Local time: the UI sends its UTC offset (`clock_sync`), because `now_local()` is unreliable in a multi-threaded process
- [x] Permissions: `AppManifest` generates `allow-<command>`, the capability grants exactly those + `core:event:allow-(un)listen`; a test keeps the command list, the capability and `generate_handler!` in sync
- [x] `app/src/lib/api/`: TS types and `invoke()` wrappers (D-7: by hand + JSON shape tests in Rust + a Vitest test comparing the wrappers with the Rust command list)

### Stage 6: UI – vault creation and lock screen
- [x] `setup`: welcome with the notice from §9, password + repeat + local strength meter (no dependency), 24 words with the V-1 warning, "I have written it down" required, retyping 3 words
- [x] `lock`: password, countdown after a wrong attempt, "I forgot my password"; `recover`: 24 words + new password
- [x] Lock button always visible when unlocked (`UnlockedShell`), notice about unreadable records
- [x] Rust events: `vault-locked` → lock screen, `privacy-curtain` → curtain; activity pings every ≥ 20 s; `clock_sync` at start and on return
- [x] i18n pl/en/uk (system language before unlocking, D-2), a test for matching keys and placeholders
- [x] Styles: calm palette, dark mode, targets ≥ 48 px, `prefers-reduced-motion`, no red in warnings
- [x] E2E (Playwright, Chromium, fake backend `e2e/fake-tauri.js`): creation, lock, wrong password with delay, unlock, auto-lock from Rust, curtain, recovery; in CI
- [ ] Ukrainian translations to be reviewed by a native speaker

### Stage 7: UI – entries
- [x] Quick entry (E-10): type + rating/intensity = 2 taps; sleep and note 1 tap; saved as "to complete", then the entry form opens straight away (complete now or leave)
- [x] Forms E-1 (weather pictograms 1–5, not emoji), E-2, E-3 (symptoms + own, avoidance, strategy, intensity after), E-4, E-6; saving after one field; a full save clears "to complete"
- [x] Timeline with filters (type, tags, date range, to complete only) and search in Rust; editing; deleting with an inline confirmation (no `confirm()`)
- [x] Tags (E-12): create, rename, colour from a palette, delete, merge
- [x] Clipboard: after `copy`, Rust clears the clipboard after 60 s if it still holds the copied text (`tauri-plugin-clipboard-manager`, used from Rust only; BSL-1.0 allowed in `deny.toml`)
- [x] E2E: quick entry, full form + edit + delete, tags + filter + diacritic-insensitive search; every test fails on a console error

### Stage 8: Plan for a hard moment
- [x] Plan editor (P-1), six sections, contacts with a call button; one tap from the timeline (P-2)
- [x] `core/data/crisis/pl.json` (+ `sources`, `verified_at`), `core::crisis` with file validation in tests; numbers checked on 2026-10-19 against the Ministry of Health (gov.pl) and the operators' pages
- [x] Help screen without unlocking (P-3): numbers with a "Call" button, country from the system language region (D-2), a message for countries without data
- [x] `dial`: only crisis numbers and contacts saved in the plan (after unlocking); `tauri-plugin-opener` used from Rust only
- [x] Offline exercises (P-4): 4-7-8 and box breathing with an animation (none under `prefers-reduced-motion`), 5-4-3-2-1
- [x] "Plan for a hard moment / phone numbers" button on the lock screen
- [x] E2E: help from the lock screen (numbers, calling, exercises), plan (saving, calling only a saved contact)
- [ ] Exercise texts to be reviewed by a psychotherapist (SPEC §9); numbers re-checked before every release

### Stage 9: `.mossbook` backup
- [x] `core::backup`: `MOSSBOOK` + version + CBOR; header and encrypted records copied 1:1 from the database; a manifest (SHA-256 of the records, sealed under the DEK) catches removal, addition and reordering; 512 MiB limit
- [x] Opens with the password or recovery key valid when the backup was made (D-9); restoring on a new device without re-encryption; importing into an existing journal with re-encryption (D-4): merge (newer `updated_at` wins, tags matched by name, plan and settings unchanged) or replace, in one transaction
- [x] `backup_export` / `backup_pick` / `backup_import` with system dialogs (plugins `dialog` + `fs`, `content://` handled for Android); the background lock is paused during the dialog (D-6) and the session mutex is not held meanwhile
- [x] Settings screen: backup (save, load), auto-lock (V-3), password change (V-5), deleting all data (V-9); "I have a backup" on the first screen
- [x] `docs/backup-format.md` + the independent reader `tools/read-backup.py` (libsodium, argon2-cffi, cbor2), checked against the fixture
- [x] Tests: export → restore / import into another vault = identical data, merge, wrong password/key, tampering (byte, removal, swap, foreign manifest, truncated file, version), old password after a change, fixture `backup-v1.mossbook`; E2E: restore, settings, password change, deletion

### Stage 10: Chart and i18n
- [x] A-1: "Patterns" tab, mood chart (week / month / 3 months) as hand-written SVG instead of uPlot (D-12): 2 px line, ringed markers, grid, crosshair + tooltip (mouse and arrow keys), table view; series colours validated for light and dark; observations only
- [x] Language setting: "as in the system", Polski, English, Українська; `Settings.locale` optional (old records with a string still load), applied after unlocking (D-2)
- [x] Texts with numbers rephrased so they need no plural forms

### Stage 11: Android
Resumed 2026-10-21 (postponed on 2026-10-19 while the tools were missing).
- [x] `cargo tauri android init`; the project in `app/src-tauri/gen/android` is committed, with the Mossbook launcher icons
- [x] Release manifest (`src/release/AndroidManifest.xml`) removes `INTERNET` and `ACCESS_NETWORK_STATE` with `tools:node="remove"`; the debug build keeps `INTERNET` for the dev server
- [x] `allowBackup="false"`, `fullBackupContent="false"`, `dataExtractionRules` excluding every domain from cloud backup and device transfer; the FileProvider exposes only the cache
- [x] `FLAG_SECURE` in `MainActivity` (no screenshots, recordings or app-switcher previews)
- [x] Edge-to-edge: the web view gets no safe-area insets, so `MainActivity` pads the content by the system bars, the display cutout and the keyboard; the window background matches the page
- [x] The keyboard is asked not to learn from anything typed (`IME_FLAG_NO_PERSONALIZED_LEARNING` on every input connection, via wry's `WRY_RUSTWEBVIEW_CLASS_EXTENSION` in `.cargo/config.toml`)
- [x] "Show in folder" for automatic backups is desktop only (`BackupInfo.can_reveal`); on Android they stay in the app's private storage
- [x] Tests: `android_project_keeps_the_hardening` (source manifests, backup rules, `FLAG_SECURE`); CI builds a release APK and `tools/check-android-apk.sh` checks the merged manifest (no network permissions, backups off) and that the native library imports no socket functions
- [x] Emulator (Android 16, Pixel 7): create a journal, quick entry, lock on going to the background, unlock, save a backup through the system dialog (opened by `tools/read-backup.py`), crisis numbers open the dialer without calling; app files and WebView storage hold no typed secrets
- [x] Test on a real phone (Redmi Note 13 Pro, Android 16, wireless debugging)
- [x] Release signing from GitHub secrets (`keystore.properties`, never committed); the `Release` workflow builds a signed APK (arm64, armv7, x86_64), checks it (`check-android-apk.sh`, `apksigner verify`) and uploads it before `SHA256SUMS`; skipped with a notice without the key; checked locally with a throwaway key
- [x] Website: Android download, installation steps, fingerprint unlock and Android privacy texts, rendered only when the release has an APK
- [x] Store metadata in `fastlane/metadata/android` (en-US, pl-PL): descriptions, screenshots, icon, changelog for version code 2000
- [x] Signing key generated and kept by the maintainer; secrets set; certificate fingerprint in `SECURITY.md` and pinned in the `Release` workflow
- [x] Release 0.2.0 with the signed APK, tested on a Redmi Note 13 Pro before publishing: install, fingerprint unlock after R8, black screenshots and app-switcher preview
- [x] Distribution: the signed APK on GitHub Releases and the website (updates by hand or with Obtainium). App stores (IzzyOnDroid, F-Droid) are not pursued for now (maintainer's decision, 2026-10-21); `fastlane/metadata/android` stays ready if that changes

### Stage 12: Release 0.1 (desktop)
- [x] `SECURITY.md`, `docs/threat-model.md` (threat → protection → place in code, accepted leaks, limitations), `CONTRIBUTING.md` (DCO, rules), `CHANGELOG.md`
- [x] `Release` workflow: tag `v*` → Linux / Windows / macOS (arm and x86, the latter cross-compiled) installers + `SHA256SUMS` as a **draft** release
- [x] `docs/release.md`: checklist (crisis numbers, independent tools, versions, manual test)
- [x] Version consistency test (Cargo, `tauri.conf.json`, `package.json`)
- [x] Before the release: random UUIDv4 instead of v7 (D-13)
- [x] Manual test of the release build (.dmg install, PDF report, quick entry); `v0.1.0` published (2026-10-20)
- [x] Offline update check in Settings → About, GitHub release notifications in the README (D-15)
- [x] Homebrew tap `jakub-m-arch/homebrew-mossbook`, updated by the `Homebrew` workflow on publish; clears quarantine because the app is not notarized
- [x] `v0.1.1`: macOS bundle signed ad-hoc as a whole (0.1.0 was reported as "damaged" after download)
- [x] `v0.1.2`: automatic backups, recovery key check, JSON export, first release with provenance attestations
- [x] `v0.1.3`: optional password for the therapist PDF (AES-256), website on GitHub Pages
- [x] `v0.2.0`: first Android release (signed APK), fingerprint unlock (2026-10-21)
- [x] `v0.3.0`: an entry form survives a lock (E-17, D-20); licence and no-warranty notice in the app (D-19) (2026-10-21)
- [x] `v0.4.0`: voice notes (E-18, D-21), sync between the user's devices through Syncthing (D-22), readable copy as ZIP, vault and backup format 3; tested as `v0.4.0-rc.1` (2026-10-22)
- [x] `v0.5.0`: voice notes to text on the device (E-19, D-23), UI polish (D-24: confirmations in a sheet, hiding content by default, entry menu, settings screens); tested as `v0.5.0-rc.1`–`rc.3` (2026-10-23)
- [ ] Enable private vulnerability reporting in the repository settings
- [ ] Code signing: Apple Developer ID + notarisation (macOS), SignPath or Azure Trusted Signing (Windows)

### Stage 14: Trust before a wider release
- [x] **Automatic backups (D-16)**: on lock, at most every 20 h, newest 7 in `backups/` next to the vault; fixed slot names; replaced after a password change, deleted with the journal; can be turned off; the backup picker opens in that folder
- [x] Settings show the latest automatic backup ("Show in folder") and when the user last saved a backup themselves (`Settings.last_export`, set only by the journal)
- [x] **Recovery key check**: type the 24 words, see whether they open this journal; nothing changes
- [x] Build provenance attestations in the `Release` workflow; all GitHub Actions pinned to commit SHAs, kept current by Dependabot (monthly, minor and patch only)
- [x] Website in `site/` (Polish root, English at `/en/`), published to GitHub Pages by the `Website` workflow with the latest release version; no third-party requests (CSP). Plain-language privacy, honest limits, downloads per system, FAQ, crisis numbers
- [x] Screenshots from the app with synthetic entries, light and dark (`app/site-shots`, `playwright.site.config.ts`); also in the README
- [x] **Legal hygiene (D-19)**: public texts without health claims, condition names as the audience or named clinical methods; the GPL no-warranty and "not a medical device" notice under Settings → About; all licence texts in the app (`tools/licenses.py`, checked in CI)
- [x] **Unsaved entry form (E-17, D-20)**: kept in the encrypted settings while typing (pause of 0.8 s, hidden window, before a manual lock); cleared by saving, discarded by leaving the form on purpose; offered on the timeline after unlocking (continue / discard with confirmation)
- [x] **Background lock with fingerprint unlock (Android)**: the lock screen asked for the fingerprint while the app was already going to the background; `BiometricPrompt` ignores a call after `onSaveInstanceState` without calling back, so `vault_unlock_device` waited forever and, wrapped in `with_dialog`, kept the background lock suspended after the next unlock. Fixed in three places: the lock screen asks only when the page is visible, the plugin answers `cancelled` once the state is saved, and `vault_unlock_device` no longer suspends the background lock (the journal is locked during the prompt). Reproduced and checked on the emulator (release build, R8)
- [ ] Crisis numbers for more countries (UA, GB, DE, US at least), from official sources
- [ ] Reviews: a psychotherapist (exercises, IRT, tone), a native Ukrainian speaker

### Stage 19: Recording while the screen turns off (D-25)
- [x] Tauri: `voice_recording(active)`; `Session::set_recording`, `on_background(now)` defers the lock while recording, `on_tick` locks once the grace is over and keeps the idle timer away while recording; a thread runs the deferred lock after the grace; every lock clears the screen flag. Tests: idle timer and its limit, the lock waiting for the UI, the lock after the grace without the UI, nothing carried to the next unlock, other policies unchanged, recording needs an open journal
- [x] Android: `ScreenPlugin.kt` (`keepOn`), registered by `src/screen.rs`; no permission needed
- [x] UI: the voice field reports the start once the microphone runs and the end after the recording is stored and the form kept (`EntryForm` awaits the keep); cancel and leaving the form report the end. e2e with the fake backend: the screen flag only while recording; the screen turning off mid-recording locks after the recording is kept, and the entry comes back with it after unlocking (fails without the fix)
- [ ] On a real phone: the screen stays on during a 2-minute recording; the power button mid-recording keeps what was said

### Stage 18: UI polish before 0.5.0 (D-24)
- [x] `ConfirmAction` opens a modal sheet (question as title, explanation below, focus on Cancel, working state); every confirmation in the app uses it
- [x] "Hiding content" instead of "difficult content", on by default in the form; the report no longer forces a password
- [x] Delete next to Save in the entry form
- [x] Entry menu on the timeline: long press or right click → open, hide or stop hiding, delete (with confirmation)
- [x] Timeline: a hidden entry shows only its type, time and a crossed-out eye (no mood, intensity or "show content" link, which revealed only two lines); tapping opens it. Sleep entries show length, quality and awakenings instead of an empty card
- [x] Text fields about three lines high, growing with the text where the web view supports `field-sizing` (Chromium)
- [x] Settings split into screens (the user's request): a hub with Report and Tags, then Language and lock, Backups and sync, Voice notes, Password and recovery key, About, and Delete all data on its own; each screen has a back button; `createMessages()` shares the per-section message handling
- [x] e2e: dialogs, defaults, the menu by touch and by right click
- [ ] On a real phone: long press in the Android web view (no text selection or system menu)

### Stage 17: Transcribing voice notes (E-19, D-23)
- [x] Spike (2026-10-22, desktop only, outside the repo): Parakeet TDT 0.6B v3 int8 through `sherpa-onnx` 1.13.8 transcribes synthetic Polish speech well (a few wrong word endings) and Ukrainian without errors; 30 s of silence or noise give an empty text (nothing invented); 5 minutes in one pass take 3.7 GB of RAM and lose quality, in pieces of 15–25 s cut at the quietest 100 ms they take 1.5 GB and 22 s on an M-series Mac (4 threads). The pure-Rust `opus-decoder` 0.1.1 panicked (shift overflow) on the web view fixtures in a debug build, so decoding uses libopus (`opus` → `opusic-sys`, built from source with CMake)
- [x] Core: `VoiceNote.transcript` (left out when empty), searchable, readable copy version 3; `transcribe/`: Ogg Opus → 16 kHz mono PCM (packets split by lacing in `domain/voice.rs`), cutting into pieces, model import (size + SHA-256 of the upstream archive, only the four needed files, by name, staging folder + rename), check and removal, the recognizer with a cancel flag and progress; tests incl. negative ones (wrong, damaged, truncated, padded or foreign archive, wrong sizes, missing or repeated files, paths outside the folder, cancel, damaged model on disk); the real archive and model in ignored tests (`MOSSBOOK_MODEL_ARCHIVE`, `MOSSBOOK_MODEL_ROOT`)
- [x] Build: the sherpa-onnx archives pinned by SHA-256 (`tools/sherpa-onnx.sha256`, matching the digests GitHub publishes), fetched and checked by `tools/fetch-sherpa-onnx.sh`, handed to the build script through `SHERPA_ONNX_ARCHIVE_DIR` so it never downloads; `deny.toml` allows `ureq`/`rustls` only on that build path (plus the zip 2 advisories and the `webpki-roots` data licence there, and the bzip2 licence of `libbz2-rs-sys`); `.cargo/audit.toml`; licence notices for the native code in the prebuilt libraries (`core/data/licenses`). Android: the build script cannot find the Tauri project in this workspace, so the fetch script unpacks the C API and ONNX Runtime into `target/sherpa-onnx-android/jniLibs`, Gradle packages them, and `check-android-apk.sh` now fails if a library a native library needs is missing; CMake needs `ANDROID_NDK_ROOT`. Checked locally: arm64 release APK builds, passes the checks (no socket imports in the new libraries)
- [x] Tauri: `transcription_status`, `transcription_open_download`, `transcription_import`, `transcription_remove`, `transcription_cancel`, `voice_transcribe`; one job at a time; a job never holds the session; a lock seen between pieces cancels it and drops the text; progress as an event with counts only; total memory from `/proc/meminfo` (Android, Linux) for the warning under 5 GiB
- [x] UI: Settings → Turning recordings into text (off by default; explanation, sizes, memory warning, download in the browser, import with progress and cancel, remove with confirmation, model credit); "Turn into text" next to a recording where the model is imported, progress and stop, an editable transcript with a note that it may contain mistakes, a message when no words were recognised; pl/en/uk; e2e with the fake backend
- [x] Docs: SPEC E-19 and principle 1, threat model, SECURITY.md, README (how to turn it on, what it means for "zero network", build prerequisites), `backup-format.md`, `crypto.md`, `architecture.md`, CHANGELOG
- [x] Website (pl/en) after 0.5.0 was published: a section on voice notes and transcription with a new screenshot, the build-time exception under "Nie łączy się z internetem", an FAQ entry, hidden content by default, settings paths; all screenshots regenerated
- [x] CI green for `0f768a6`: Rust tests on Linux, desktop builds on Ubuntu 24.04, Windows (static MT libraries link) and macOS arm64, the arm64 APK with the extended checks
- [x] After 0.5.0 (the user's feedback, 2026-10-23): removing a recording or recording it again no longer drops its text silently; a sheet offers "Keep the text in the note", "Remove the text too" or Cancel (the text is appended to the note after a blank line). The report has "Include the text of recordings" (off by default, shown only when an entry in the selection has text) that adds it as a "Text of the recording" field. Unit test for the report, e2e for both
- [x] Three-ABI Android APK built locally (2026-10-23): ARMv7 caught a 32-bit `usize` overflow in the sample limit (fixed, with a test). Native libraries are now compressed in the APK (`useLegacyPackaging`, the user's choice over per-ABI APKs): universal APK 46 MB instead of ~100 MB stored (31 MB in 0.4.0), unpacked on install. Checks pass; on the arm64 emulator the app starts with ONNX Runtime and the sherpa-onnx C API loaded
- [x] `v0.5.0-rc.2` (2026-10-23): every release build passes (Linux on ubuntu-22.04, Windows, macOS arm64 and x86_64, the signed three-ABI APK). rc.1 lost its APK: CI restored a cached build whose sherpa-onnx build script had already run but not the folder it unpacked into, so `tools/fetch-sherpa-onnx.sh` now unpacks the archives itself
- [x] On a real phone (2026-10-23, `v0.5.0-rc.2`): model imported in the app, recordings turned into text; works well (the user's report)

### Stage 16: Sync between the user's own devices (D-22)
- [x] Core: `updated_at` on tags, plan and settings (stamped by the journal); tombstones (record type 6 at the deleted record's id) for deleted entries and tags and for what a Replace import drops; vault format 3 (migration 3) with `password_epoch`; backup version 3 (header in the manifest, AAD `mossbook/backup/v3`); a sync file is a backup opened with the shared DEK (`open_sync_file`), `adopt_password_from`, `Journal::merge_from_device` (newer wins, deterministic ties, tombstones, recordings copied, same-name tags joined); `tools/read-backup.py` reads v3; tests: both orders, deletion vs. older/newer edit, tags, plan/settings and the kept form, password change and an old file replayed, foreign and damaged files, ties
- [x] Pairing (`Session::sync_join`): the newest file of another journal in the folder is opened with that journal's password (throttled like unlocking), this journal goes to the automatic backups, is merged into the other one and replaced by it (vault id, DEK, password, recovery key); the fingerprint key file is removed; warning and confirmation in the UI
- [x] Folder on desktop: a path picked in the system dialog, kept in `meta` on this device; each device writes only its own `<device id>.mossbook-sync` (temporary dot-file, then rename), so Syncthing never sees two writers of one file; Syncthing's conflict copies and temporary files are ignored
- [x] Folder on Android: `SyncFolderPlugin` (Kotlin): the system folder picker with a persisted permission, list/read/write/remove through the Storage Access Framework, files passed through the app's cache (already encrypted), written as a dot-file and renamed; checked on the emulator (release build): pick a folder, write on connect, pair a second journal from that file, read its file on unlock
- [x] When: written on lock (skipped when the journal is unchanged, by the backup's fingerprint) and after pairing; read on unlock and when the app comes back while unlocked, only files that changed; the timeline reloads when something arrived; Settings → Sync shows the folder, the devices and when each file changed, the last sync and any problem; turning sync off keeps everything; destroying the journal removes this device's file
- [x] Docs: sync files in `backup-format.md` (a sync file is a backup), `crypto.md`, threat model (what the folder and `meta` reveal, clocks, joining), SPEC X-8, README (setting up Syncthing), CHANGELOG
- [ ] On real devices with Syncthing (a phone and a computer) before the next release

### Stage 15: Voice notes (E-18, D-21)
- [x] Manual check (2026-10-22): Android emulator (release build, R8; a format 1 journal migrated on open): record, play, save, lock in the background and unlock with the fingerprint; macOS app bundle: record and play
- [ ] On a real phone before the next release: record, play, backup and restore with recordings
- [x] Spike (branch `spike/voice-notes`, not merged): `MediaRecorder` gives WebM/Opus on Android and Windows, MP4/AAC by default on macOS and only MP4 on WebKitGTK, whose MP4 does not play on macOS; macOS ignores the bitrate (~16 KB/s AAC). WebCodecs `AudioEncoder` with Opus works in all four web views (Android, macOS, Windows, WebKitGTK), and Ogg Opus muxed in the page plays in all four, each other's files included, with the right length (fixtures in `core/tests/fixtures/voice/`). Hence one format: Opus from WebCodecs in Ogg. macOS needs `NSMicrophoneUsageDescription` (or `getUserMedia` is missing) and the `audio-input` entitlement (or the hardened runtime denies silently); WebKitGTK needs `enable-media-stream`; Android passes IPC bodies as JSON, so bytes travel as base64
- [x] Core: `media` table (SQL migration 2), record type 5 `voice`, decrypted only for playback, Ogg Opus checked by the journal (structure, CRC, ≤ 5 min, ≤ 4 MiB; length from the last granule), deleted with the entry or when replaced, orphans removed on lock/unlock/discard; tests incl. negative ones (blob under another id or among records, foreign files, over the limits, shared or missing recording)
- [x] Vault `format_version` 2 + migration test (frozen `vault-v1.db`, new `vault-v2.db`); backup version 2 (`media` array, manifest over both lists with AAD `mossbook/backup/v2`, so it cannot pass as version 1) + `backup-v2.mossbook`, version 1 still imports; import re-encrypts recordings one at a time; `tools/read-backup.py` reads both versions
- [x] Tauri: `voice_save` / `voice_get` with base64 bodies (no file on disk); the web view gets the microphone and no other permission; macOS `Info.plist` + entitlement, Android `RECORD_AUDIO`, WebKitGTK media stream, CSP `media-src blob:`; tests for the Android and macOS declarations
- [x] UI: record (microphone → AudioWorklet → WebCodecs Opus 32 kbit/s → Ogg, `src/lib/voice/`), 5:00 limit with the time shown, stop, listen, record again, remove; playback from an object URL revoked when the player goes away; a recording in progress is dropped on lock and finished when the app is hidden; the timeline shows the length; texts in pl/en/uk; e2e with Chromium's synthetic microphone
- [x] Unsaved form (E-17): a finished recording is stored at once and the form kept right away, so it survives a lock
- [x] Readable copy as ZIP (`mossbook-export.json` version 2 + `voice/<id>.ogg`, stored, no timestamps, built in a buffer sized up front; command `export_readable`), PDF report shows the length only; docs: `crypto.md`, `backup-format.md`, `threat-model.md`, CHANGELOG

### Stage 13 (v1)
- [x] **PDF report for a therapist (X-3)**: period, types and tags; the frontend builds the document model (translations), the same model is the preview and goes to `core::report`, which lays out A4 with `printpdf` (word wrap by glyph widths, pagination, footer + page number); Noto Sans (OFL) embedded whole (~600 KB; subsetting would need the heavy `text_layout` feature); a warning before saving; saved through the system dialog
- [x] **Optional PDF password (D-17)**: AES-256 (PDF 2.0, V5/R6) with `lopdf`, a generated 20-character password, a 12-character minimum, no stored password, auto-enabled for reports containing "difficult" entries, and a warning about sending the password separately
- [x] **Nightmare / IRT (E-5)**: creation (one-tap quick entry), an explanation of the method, the nightmare collapsed when reopened, the new version, rehearsals with dates; "difficult content" by default
- [x] **Difficult content (E-16)**: a switch in every form; the timeline shows only the type and time, revealed on tap
- [x] **Calendar (E-14)** in Patterns: a month, each day tinted by average mood (one hue, no red), legend, month navigation; tapping a day → timeline filtered to that day
- [x] **Overviews A-2–A-4** (`core::query::patterns`, Patterns → Overviews): from 14 entries in the period; most frequent tags and triggers (free text grouped after normalisation, ≥ 2 repeats); mood versus the previous night's sleep (poor/fair/good), time of day, day of week; strategies: average intensity drop (≥ 2 uses); bars with numbers, observations only
- [x] **Quick entry without unlocking (E-15)**: X25519 pair in the header (`x25519_pub`, `wrapped_x25519_priv`), libsodium-compatible sealed box (tested against PyNaCl), `inbox` table, minimal `InboxEntry` (type, value, time), moved into the journal on unlock; older vaults get the keys on their next unlock; button on the lock screen
- [x] **Plain JSON export (X-4)**: Settings → readable copy, after a warning; `format`/`version` envelope with the domain types, documented in `docs/backup-format.md`
- [ ] Reminders (R-1–R-3): **postponed to the mobile versions** (D-14)
- [x] **Fingerprint unlock (V-7, D-18)**: a random device key wraps the DEK (`AAD = "mossbook/wrap/device/v1" || vault_id`, vector checked with libsodium); the device key is encrypted by an AES-256-GCM Android Keystore key (StrongBox when present) bound to class 3 biometrics, `setInvalidatedByBiometricEnrollment`, a fresh check for every use; `vault.db.device-key` (format v1) next to the vault, never in backups, removed with the journal, when the key is rejected or the fingerprints changed; turned on in Settings with the password and one fingerprint; the lock screen asks for the fingerprint at once and keeps the password; a hint when the phone has a sensor but no fingerprint; Kotlin plugin `DeviceKeyPlugin` called only from Rust; tested on a Redmi Note 13 Pro (Android 16)
- [ ] iOS (postponed)

---

## 3. Decisions and gaps in the specification

Status: **accepted** or **open**. Accepted decisions bind the implementation just like
`SPEC.md`.

| ID | Status | Problem | Decision |
|---|---|---|---|
| D-1 | accepted | **Licence** | `GPL-3.0-or-later` + a section 7 permission for App Store / Google Play distribution (`LICENSE-EXCEPTION.md`). Copyleft blocks closed forks with telemetry; the permission removes the known conflict between the GPL and App Store terms. Contributions under the DCO (`Signed-off-by`). Dependencies must have GPL-3.0-compatible licences (enforced by `cargo deny`) |
| D-2 | accepted | **Settings before unlocking**: the lock screen needs a language and a crisis-line country, but `Settings` are encrypted (principle 2) | Before unlocking, language and country come from the system. The saved language applies after unlocking. No plaintext setting in `meta` |
| D-3 | accepted | **When the vault is created**: if the app closes between `vault_create` and `vault_confirm_recovery`, a vault without a confirmed key would exist | The vault is built in memory and written to disk only after the words are confirmed. `vault_create` returns the phrase, `vault_confirm_recovery` writes the file |
| D-4 | accepted | **Importing from another vault**: the AAD contains `vault_id`, so records from another vault's backup cannot simply be copied | Import always decrypts with the backup's key and re-encrypts under the current DEK. Id conflicts when merging: newer `updated_at` wins |
| D-5 | accepted | **Wrong-attempt counter (V-8)**: a persistent counter on disk is plaintext metadata, and an offline attacker bypasses it anyway | The counter lives in process memory only. The real protection is Argon2id, as the threat model says |
| D-6 | accepted | **Auto-lock and system dialogs**: a save dialog or the Android picker causes focus loss or backgrounding, which would lock the vault mid-export | A "system dialog in progress" flag in Rust pauses the "immediately" lock, but not the idle timer |
| D-7 | accepted | **TS types**: maintaining them by hand risks drift | By hand, guarded by JSON shape tests in Rust and a Vitest test comparing the wrappers with the Rust command list (`specta`/`tauri-specta` were not stable enough for Tauri 2) |
| D-8 | open | **The plaintext `type` column** reveals how many entries (as opposed to tags) there are | Accepted for now and listed among the leaks in SPEC §2 and the threat model. Alternative: a single "record" type with the type only in the AAD and content |
| D-9 | accepted | **An old backup after a password change** opens with the old password (it holds the DEK wrapped under the old KEK) | Documented in the password-change UI and in `docs/backup-format.md` |
| D-10 | accepted | **Frontend router**: SPEC mentions `routes/` | SvelteKit + `adapter-static` (the official Tauri template) |
| D-11 | accepted | **Desktop "immediately on going to the background"**: on desktop focus is lost all the time | Desktop: lock on minimise/hide; focus loss only covers the window |
| D-12 | accepted | **Chart library**: SPEC named uPlot | Hand-written SVG charts: no dependency, colours from CSS variables (dark mode without redrawing), accessibility (slider role, table view) |
| D-13 | accepted | **UUIDv7 reveals the creation time**: `records.id` is stored in plaintext (database and backup), and v7 contains a millisecond timestamp, which breaks principle 2 | New ids are random (UUIDv4); sorting uses `occurred_at` from the encrypted content. Existing v7 ids are read unchanged |
| D-14 | accepted | **Reminders on desktop**: the schedule is in the encrypted settings, yet a reminder must arrive while the journal is locked; on desktop only while the app runs | Postponed to the mobile versions (system notification scheduling). No plaintext schedule on disk |
| D-15 | accepted | **Updates without network**: auto-update breaks principle 1 (a connection reveals the IP address and that the app is used) | Settings → About: version, build date (`SOURCE_DATE_EPOCH` for reproducibility), a hint after 90 days computed locally, a button opening the Releases page in the browser (fixed URL in Rust); README: GitHub notifications. Package managers (Homebrew; later winget, Flathub) update from outside |
| D-16 | accepted | **Losing the only copy**: a damaged `vault.db` or a mistake (deleting, a bad import) loses everything unless the user saved a backup | Automatic backups when the journal locks, at most every 20 h, newest 7, in `backups/` next to the vault, same format and encryption as manual backups. Fixed slot names (no dates in plaintext); file times reveal on which recent days the journal was locked (accepted, documented). Replaced after a password change so they stop opening with the old password; deleted with the journal; can be turned off |
| D-18 | accepted | **Fingerprint unlock (V-7)**: where to keep the device-bound wrap, and whether to ask for the password from time to time | A separate file `vault.db.device-key` (no change to the vault format, never in backups, useless on another device). Its device key is released by the Android Keystore only after a class 3 biometric check; changed fingerprints destroy it. No periodic password prompt (the user's choice, 2026-10-21); Settings remind that the password and the 24 words are the only way to open the journal elsewhere. Not throttled by Mossbook: the system limits attempts. Face unlock that is not class 3 is not offered |
| D-17 | accepted | **The PDF report is a deliberate plaintext export**: the therapist must be able to open it, but the file may travel by email or sit in a download folder | Optional password protection (AES-256, PDF 2.0 V5/R6) on top of the existing warning; the password is entered or generated in the UI, never stored, and must be sent through a different channel. The PDF security handler's KDF is weaker than Argon2id, so a 12-character minimum is enforced and the UI recommends a generated password. Not a substitute for choosing what goes into the report or for the encrypted backup; the password does not protect against the therapist's device, screenshots or printing |
| D-19 | accepted | **Legal exposure of a free well-being app**: health claims could make it a medical device (MDR, intended purpose), absolute privacy promises could mislead, and permissive licences require their notices in the binary | Public texts describe a journal, never treatment: no condition names as the target group, no clinical method names or references to proprietary clinical forms (the plan for a hard moment is described as our own, with our own wording; the authors of published safety-plan forms are not contacted), no efficacy claims, "not a medical device and not a substitute for professional help" next to every description. "Nobody but you can read" became "your entries stay with you", with the limits linked. The app shows the GPL no-warranty notice and every licence text (Rust, npm, fonts) under Settings → About; `tools/licenses.py --check` runs in CI, so a dependency change must regenerate `app/src/lib/assets/licenses.txt`. Not legal advice; a lawyer review is due before any paid or commercial distribution (product liability, CRA) |
| D-20 | accepted | **An entry form lost on lock**: with the default "lock immediately", a phone call or switching apps while writing locked the journal and dropped the open form | The form is kept, encrypted, as `Settings.unsaved_entry` (one at a time): a field with a default is not a format change (`docs/crypto.md`), older builds ignore it, backups carry it with the settings, the JSON export leaves it out. The journal sets it (`keep_unsaved_entry`), `save_settings` cannot set or erase it, saving the entry clears it. The UI keeps the form after a 0.8 s pause, on `visibilitychange` and before a manual lock; leaving the form for another screen discards it, the lock screen keeps it. A separate record type was rejected: it would show as an unreadable record in older builds or need a format bump |
| D-21 | accepted | **Voice notes (E-18)**: audio is ~100× larger than text, while unlocking decrypts every record into memory, and backups, the inbox and the JSON export were designed for small records | Voice only, no video or other files (the user's choice, 2026-10-21). At most **5 minutes**, recorded inside the app (no import from the gallery or files, which would leave a plaintext copy outside the journal), ~32 kbit/s Opus or AAC, so ≤ ~1.2 MB; larger blobs are refused. Each recording is one AEAD blob (no streaming needed at this size) with the record AAD and `record_type = "voice"` (5), stored in a **separate `media` table**: unlocking reads only `records`; a recording is decrypted only when played. `Entry.voice: Option<VoiceRef { id, duration_ms, mime }>` (a field with a default). Deleting the entry deletes the recording; recordings no entry or unsaved form points to are removed on lock. Vault `format_version` 2 (a new table: older builds refuse the file instead of counting unknown rows); backup body version 2 with a `media` array covered by the manifest; **recordings are in backups like notes** (the user's choice), so automatic backups grow with them. Playback: Rust → raw bytes over IPC → `Blob` → object URL in the web view, revoked on lock; never a file on disk (the web view copy cannot be zeroed, like typed text). Not offered for quick entry while locked (E-15). PDF report: "voice note, 2:31", no audio. **Readable copy becomes a ZIP** (the user's choice): `mossbook-export.json` plus `voice/<entry id>.<ext>`, export format version 2; files are stored uncompressed (audio is already compressed), the archive is built in memory and written only to the place the user picks, never to a temporary directory; the warning before export says it holds plaintext recordings of the user's voice. **Recording on every platform** (the user's choice): through `MediaRecorder` in the web view where the spike shows it works; where it does not (risk: WebKitGTK on Linux), native capture and Opus encoding in Rust, after checking the crates' licences and that they open no network connections |
| D-22 | accepted | **Same journal on the phone and the computer**: two devices hold separate journals with separate keys, and Mossbook must not talk to the network (principle 1) | The user's choice (2026-10-22): files carried by **Syncthing** between their own devices; Mossbook never connects anywhere, it only reads and writes a folder. Both devices become **one journal** (same vault id and DEK), so a sync needs no password. **A sync file is a backup** (same format, manifest and checks) named `<device id>.mossbook-sync`; each device writes only its own, atomically, and merges the others' with the shared DEK. Pairing: the device where sync is turned on first keeps its password and recovery key; the second one opens the first one's file with that password after a clear warning, merges its own journal in, takes over the identity, and keeps its previous journal as an automatic backup; fingerprint unlock is set up again. Merge: **the newer edit wins** per entry, tag, plan and settings (`updated_at`, ties broken by the encoded bytes so all devices agree); deletions travel as encrypted **tombstones** (record type 6 at the deleted record's id); two tags with the same name become one (the smaller id stays). **Plan and settings are synced too**, except the unsaved form (E-17) and what belongs to a device (fingerprint key, inbox key, the folder). A password change travels with the header: `password_epoch` in `meta` counts changes and the higher one is adopted. Vault format 3 and backup version 3 (tombstones; old builds refuse them). Clocks: `updated_at` comes from each device's clock, so a wrong clock can make an older edit win; accepted and documented. The folder reveals the number of devices, the size of each file and when it changed |
| D-23 | accepted | **Transcribing voice notes (E-19)**: speech recognition that works offline needs a model of hundreds of MB and a native inference engine, while the installer should stay small and Mossbook never connects to the network | The user's choices (2026-10-22): **off by default**, turned on per device by importing the model. **Model**: NVIDIA Parakeet TDT 0.6B v3, int8 (CC BY 4.0; pl, uk, en among 25 languages; punctuation; a transducer, so it does not invent text on silence like Whisper), the upstream sherpa-onnx archive (487 MB, ~670 MB unpacked) pinned by size and SHA-256. **Download**: the app opens the fixed address in the system browser and the user picks the downloaded file; Mossbook itself makes no connection (an in-app download was rejected: it would need a network permission and an HTTP client and would tell a server who uses Mossbook). The model lives in the app data folder of that device, outside the journal, backups and sync; removing it turns the feature off. **Engine**: `sherpa-onnx` with onnxruntime, prebuilt by the sherpa-onnx project and shipped inside the app (native code is never downloaded at run time); installers grow by ~10–30 MB. Its build script can download those libraries, so `ureq` is allowed in `deny.toml` only as that build dependency and builds hand it archives checked against pinned SHA-256 (the compromise is stated in README, SECURITY.md, the threat model and on the website). **Running**: on request per recording, Ogg Opus decoded with libopus to 16 kHz, cut into 15–25 s pieces at quiet points (5 minutes in one pass needed 3.7 GB), the model loaded per job and dropped after it (~1.5 GB of RAM; a warning on devices under 6 GB); a lock cancels the job. **Result**: a proposal the user reads and edits, stored as `VoiceNote.transcript` (left out when empty, so not a format change; an older build drops it when it saves the entry), searched like other text, in the readable copy (export version 3); the PDF report shows only the length unless the user ticks "Include the text of recordings" (off by default; 2026-10-23, the user's choice). **Removing or recording again** a recording with text asks whether the text moves to the entry's note or goes with the recording (the user's choice, 2026-10-23); when recording again it moves only once the new recording is stored. The text never outlives its recording as a field of its own, so no format change. **Licences**: the prebuilt static libraries include eSpeak NG (GPL-3.0-or-later, for sherpa-onnx's speech synthesis, unused here), compatible with Mossbook's licence but not covered by its App Store permission (`LICENSE-EXCEPTION.md`): an iOS build would need sherpa-onnx built without TTS |
| D-24 | accepted | **UI polish before 0.5.0**: confirmations unfolded under their button, easy to miss on a phone; "difficult content" read as a judgement about the entry; deleting sat alone at the bottom of the form; the timeline had no per-entry actions | The user's choices (2026-10-23). Confirmations open in a modal sheet (the existing `Sheet`: bottom sheet on phones, dialog on wide screens), title = the question, focus on Cancel. The option is renamed **"Hiding content"** and is **on by default for entries written in the form**; quick entries (a single number) stay visible, nightmares stay hidden. Hiding is about the timeline only: the PDF report no longer turns its password on by itself for such entries (that part of D-17 is withdrawn), since almost every report would get one. Delete sits next to Save as an icon button. **Settings** become a hub of groups, each on its own screen. **Entry menu**: a long press (touch, 450 ms, the card presses in) or a right click opens Open / Hide or stop hiding content / Delete |
| D-25 | accepted | **A recording lost when the screen turns off**: the phone's screen timeout sent the app to the background mid-recording; under "lock immediately" the journal locked before the recording was encoded and stored, and the idle timer (1 min under that policy) could lock it while someone spoke without touching the screen | The user's choices (2026-10-23). **The screen stays on while recording** (Android `FLAG_KEEP_SCREEN_ON` through `ScreenPlugin.kt`, called only from Rust; set on start, cleared on stop, cancel, leaving the form and every lock; desktop does nothing, as a dark screen does not lock there). **Recording counts as activity**: the idle timer waits, for at most 5 min 30 s from the start, so a flag the UI never clears stops counting. **A background lock waits for the recording**: up to 3 s (`RECORDING_GRACE`), until the UI reports the end, which it does only once the recording is stored and the form kept (D-20), since a recording no form points to is removed on lock; then the lock happens as before. The cost, accepted: the key stays in memory for at most 3 s while the app is in the background. Saving the recording in pieces while recording (lock stays immediate) was rejected as much more work in the core |

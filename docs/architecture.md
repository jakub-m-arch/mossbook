# Architecture

How the code is organised and how data moves through it. The requirements are in
[`SPEC.md`](../SPEC.md) §8, the cryptography in [`crypto.md`](crypto.md), and what this
protects against in [`threat-model.md`](threat-model.md).

## Layers

```
┌───────────────────────────────────────────┐
│ app/src         Svelte 5 + TypeScript     │  presentation only; never sees a key
│                 lib/api: typed invoke()   │
└─────────────────────┬─────────────────────┘
                      │ Tauri IPC (only the app's own commands are granted)
┌─────────────────────▼─────────────────────┐
│ app/src-tauri   mossbook (thin layer)     │  commands, Session state machine,
│                                           │  auto-lock, system dialogs, plugins
└─────────────────────┬─────────────────────┘
                      │ plain Rust calls
┌─────────────────────▼─────────────────────┐
│ core            mossbook-core (no Tauri)  │  crypto, vault, store, domain,
│                                           │  journal, queries, backup, report
└─────────────────────┬─────────────────────┘
                      │ encrypted blobs only
                 vault.db (SQLite)
```

The dependency direction is strict: `core` knows nothing about Tauri or the UI and is
tested on its own; `app/src-tauri` depends on `core`; the UI depends only on the
command contract.

## Core (`core/src`)

| Module | Responsibility |
|---|---|
| `crypto/` | The only place that touches primitives: `kdf` (Argon2id, calibration, parameter bounds), `aead` (record encryption and AAD), `wrap` (DEK wrapping), `recovery` (BIP-39 + HKDF), `sealed` (X25519 sealed boxes for the inbox). Key types live in `SecretBox` and have redacted `Debug`. |
| `store/` | SQLite access: opening with WAL and `secure_delete`, migrations by `user_version`, raw rows of `meta`, `records` and `inbox`. Knows bytes, not meaning. |
| `vault/` | The vault life cycle on top of `store` and `crypto`: `PendingVault` (create in memory, write after the recovery words are confirmed), `unlock` / `unlock_with_recovery`, `UnlockedVault` (holds the DEK, encrypts on `put` / `write_batch`), `change_password` (rewrap), `destroy`, `append_to_inbox` (works while locked), and the in-memory `UnlockThrottle`. |
| `domain/` | Plain data types: `Entry`, `EntryKind`, `EntryDraft`, `InboxEntry`, `Tag`, `SafetyPlan`, `Settings`, `Id`, `Timestamp`. CBOR encoding, range validation, `ZeroizeOnDrop`, redacted `Debug`. |
| `journal.rs` | `Journal`: the unlocked journal as domain objects (`HashMap`s of entries and tags, the plan, the settings) on top of an `UnlockedVault`. All mutations go through it: save, quick entry, tags (merge and delete in one transaction), import, and moving inbox entries in on open. |
| `query/` | Read-only computations over the in-memory journal: `filter`, `search` (case- and diacritic-insensitive), `stats` (mood series), `patterns` (overviews A-2–A-4, with minimum data thresholds). |
| `backup/` | The `.mossbook` file: export (header + encrypted records 1:1 + sealed manifest), open with password or recovery key, restore without re-encryption, decode for import. `auto`: automatic backups on lock (slots, rotation, clearing; D-16). |
| `report/` | The therapist PDF: takes a `ReportDoc` built by the UI and lays it out on A4 with embedded Noto Sans. |
| `crisis.rs` | Static crisis phone numbers from `core/data/crisis/*.json`, embedded at compile time. |

## App layer (`app/src-tauri/src`)

| File | Responsibility |
|---|---|
| `lib.rs` | Builder: plugins (clipboard, opener, dialog, fs; used from Rust only), `AppState`, the handler list, window events, the idle timer thread, locking on exit. |
| `session.rs` | `Session`: the state machine `NoVault → Creating(PendingVault) → Locked ⇄ Unlocked(Journal)`, plus the throttle, the auto-lock policy, the UTC offset from the UI and the "system dialog in progress" flag. Plain Rust, unit-tested without Tauri. |
| `commands.rs` | One `#[tauri::command(async)]` per operation; each locks the session, calls the core and maps errors. Passwords are moved into `Zeroizing` at once. |
| `error.rs` | `AppError { code, message, retry_after_ms }`: codes, never user data. |
| `command_names.rs` | The single command list, shared with `build.rs` (permission manifest) and the tests that check the capability file and the handler. |

`AppState` is a `Mutex<Session>`. Long operations that open a system dialog (backup,
report) do not hold the mutex while the dialog is open.

### Auto-lock

The decision is always made in Rust:

- **Background**: minimising the window (desktop) or `Suspended` (mobile) locks under
  the "immediately" policy. Losing focus only emits `privacy-curtain`, which covers the
  content (D-11).
- **Idle**: a thread checks every 5 s; the UI resets the timer with `activity_ping`
  (at most every 20 s).
- **System dialogs** pause the background lock but not the idle timer (D-6).
- **Exit** locks the vault (WAL checkpoint, keys and content zeroed).

When the vault locks, Rust emits `vault-locked` and the UI navigates to the lock
screen.

### Permissions

`build.rs` generates an `allow-<command>` permission for each name in
`command_names.rs`; `capabilities/default.json` grants exactly those, plus listening to
events. No plugin's JavaScript commands are granted. The CSP allows only IPC.

## UI (`app/src`)

- **SvelteKit static SPA** (`ssr = false`, `adapter-static`), Svelte 5 runes,
  TypeScript.
- `lib/api/`: `types.ts` mirrors the Rust types; `index.ts` has one wrapper per
  command. A test checks the wrapper list against the Rust command list, and Rust tests
  pin the JSON shapes.
- `lib/i18n/`: `pl.json`, `en.json`, `uk.json`; a test checks that keys and
  placeholders match. Before unlocking the system language is used (D-2).
- `lib/session.svelte.ts`: clock sync, activity pings, the curtain and lock events.
- `lib/components/`: shared pieces (unlocked shell with the tab bar, quick entry,
  entry form, charts, calendar, sheets).
- `routes/`: `setup`, `lock`, `recover`, `restore`, `help` (crisis lines and exercises,
  no unlocking needed), `timeline`, `entry/new`, `entry/[id]`, `tags`, `plan`,
  `patterns`, `report`, `settings`.

The UI never builds ids or timestamps and never sees keys: it sends drafts and receives
decrypted domain objects.

## Main flows

**Unlock.** `vault_unlock(password)` → throttle check → `vault::unlock` reads the
header, derives `KEK_pwd` with Argon2id, unwraps the DEK, decrypts every record
(unreadable ones are counted, not fatal) → `Journal::open` decodes them into domain
objects and moves inbox entries into the journal → the session becomes `Unlocked`.

**Save an entry.** The UI sends an `EntryDraft` → `Journal::save_draft` validates it,
sets ids and timestamps, encodes CBOR → `UnlockedVault::put` encrypts with a fresh
nonce and the record AAD → one SQLite write.

**Quick entry while locked.** `quick_entry_locked` → `vault::append_to_inbox` seals an
`InboxEntry` to the vault's public key and inserts it into `inbox`. Nothing is read.

**Backup.** `backup_export` opens the save dialog → `UnlockedVault::export_backup`
copies the header and encrypted records and seals a manifest → file written.
`backup_pick` + `backup_import` open a backup with its own secret and either restore it
(no vault yet) or merge/replace (re-encrypting under the current DEK, D-4).
`export_json` writes the unencrypted JSON export (X-4) after the UI has warned.

**Lock.** Manual, background, idle or exit → if an automatic backup is due (D-16), one
is written to `backups/` (best effort: a failure never blocks locking) → the `Journal`
is dropped: the DEK and all
decrypted objects are zeroed, the WAL is checkpointed.

## Tests

| Where | What |
|---|---|
| `core` unit tests | Crypto round-trips and tampering, KDF bounds, domain validation, queries |
| `core/tests/` | Vault life cycle, journal, inbox, backups, committed format fixtures (`vault-v1.db`, `backup-v1.mossbook`), property tests |
| `app/src-tauri` tests | Session state machine, auto-lock policy, command/capability/handler consistency, JSON shapes, version consistency |
| `app/src/**/*.spec.ts` | UI logic (Vitest), i18n parity, API wrapper list |
| `app/e2e/` | Playwright scenarios against `fake-tauri.js`, an in-browser stand-in for the backend; any console error fails a test |
| `tools/` | Independent Python readers that check the vectors and the backup format without Mossbook code |

## Build and release

- CI (`.github/workflows/ci.yml`): fmt, clippy, tests, `cargo deny`, `cargo audit`,
  frontend check/lint/tests/e2e, desktop builds on three systems.
- Release (`release.yml`): a `v*` tag builds the installers and `SHA256SUMS` into a
  draft release. Publishing it triggers `homebrew.yml`, which updates the cask. The
  checklist is [`release.md`](release.md).

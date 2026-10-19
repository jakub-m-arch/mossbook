# Mossbook

[![CI](https://github.com/jakub-m-arch/mossbook/actions/workflows/ci.yml/badge.svg)](https://github.com/jakub-m-arch/mossbook/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-4f6b47)](LICENSE)

**A private, offline journal for mood, events, fears, sleep and nightmares.**

Mossbook is built for everyone, with particular care for people living with PTSD or
anxiety and for people working with a therapist. Notes are written in seconds, kept
encrypted on your device, and can be shared with a therapist only if you choose to.

> **Nobody but you can read your entries.** There is no server, no account and no
> telemetry. Everything is encrypted on your device, and the code is open so anyone
> can check.

> [!IMPORTANT]
> **Mossbook is not a medical device.** It does not diagnose, recommend treatment or
> assess your health. It is a tool for writing down and organising your own
> observations. If you are in danger, call your local emergency number (112 in the EU).

---

## Status

**Pre-alpha, not ready for real data yet.** The encrypted core is complete and
heavily tested; journaling, tags, the safety plan and crisis lines work in the
interface, and so do encrypted backups. There are no releases yet.

| Stage | Scope | State |
|---|---|---|
| 1 | Workspace, Tauri 2 + SvelteKit skeleton, CI | done |
| 2 | Cryptography (Argon2id, XChaCha20-Poly1305, recovery key) | done |
| 3 | Vault file and SQLite storage | done |
| 4 | Domain model, journal, search, mood statistics | done |
| 5 | Tauri commands, session state, auto-lock | done |
| 6 | Setup, lock and recovery screens | done |
| 7 | Quick entry, entry forms, timeline | done |
| 8 | Plan for a hard moment, crisis lines, grounding exercises | done |
| 9 | Encrypted `.mossbook` backup: export and import | done |
| 10 | Mood chart, full translations (pl / en / uk) | next |
| 11 | Android build | planned |
| 12 | Release 0.1.0 | planned |

The detailed checklist lives in [`docs/PLAN.md`](docs/PLAN.md) and the full
specification in [`SPEC.md`](SPEC.md) (both in Polish).

## What it will do

- **Entry types**: mood (1–5 with emotions), event, fear or trigger (intensity before
  and after a coping strategy), sleep, nightmare with a rescripted version (IRT), free note.
- **Quick entry** in at most two taps, to be completed later.
- **Timeline** with filters by type, tag and date, and full-text search that ignores
  case and diacritics (searching `lodz` finds `Łódź`).
- **Plan for a hard moment** based on the Stanley–Brown safety plan, with crisis
  phone numbers and grounding exercises available **without unlocking**.
- **Patterns**: mood over time, shown as plain observations, never clinical conclusions.
- **Encrypted backup** to a single `.mossbook` file, and later a PDF report for your
  therapist (you choose the period, types and tags).
- **Languages**: Polish, English and Ukrainian from the first release.
- **Calm by design**: no streaks, points, badges or guilt-inducing notifications.

## Privacy and security

| Principle | How |
|---|---|
| Zero network | No networking code or dependencies (enforced by `cargo deny`). The Android release build will not even hold the `INTERNET` permission. |
| Everything encrypted at rest | Every record, tag, date and setting is encrypted with XChaCha20-Poly1305. Nothing readable is stored on disk. |
| Strong password hashing | The key is derived from your password with Argon2id (64 MiB, calibrated to ~0.75 s on your device). |
| Recovery key | 24 words (BIP-39) generated when you create the journal. Without your password **and** this key, nobody can recover your entries, including us. |
| Tamper detection | Each record is authenticated and bound to its id, type and vault, so swapped or altered records are detected. |
| Auto-lock | Immediately when the app goes to the background (default), or after 1, 5 or 15 minutes. The decision is made in Rust, not in the web view. |
| No leaks to logs | Logging is forbidden by lints; types holding your text have redacted `Debug` output and are wiped from memory when the journal locks. |
| No telemetry | None, not even "anonymous" analytics or crash reports. |

Out of scope: a device with malware or root access, keyloggers, physical coercion, and
memory analysis while the journal is unlocked.

The cryptographic design is documented in [`docs/crypto.md`](docs/crypto.md) and the
backup file in [`docs/backup-format.md`](docs/backup-format.md), in enough detail to
write an independent reader. The test vectors are cross-checked against libsodium and
argon2-cffi by [`tools/verify-vectors.py`](tools/verify-vectors.py), and
[`tools/read-backup.py`](tools/read-backup.py) opens backups without any Mossbook code.

**Found a security issue?** Please do not open a public issue. Use GitHub's
[private vulnerability reporting](https://github.com/jakub-m-arch/mossbook/security/advisories/new).

## Architecture

```
┌──────────────────────────────┐
│  Svelte 5 + TypeScript (UI)  │  presentation only, never sees a key
└──────────────┬───────────────┘
               │ typed Tauri commands (app/src/lib/api)
┌──────────────▼───────────────┐
│  app/src-tauri               │  thin layer: commands, session state, auto-lock
└──────────────┬───────────────┘
┌──────────────▼───────────────┐
│  core (mossbook-core)        │  pure Rust, no Tauri: crypto, vault, domain, queries
└──────────────┬───────────────┘
               │ encrypted blobs only
          vault.db (SQLite)
```

- **[Tauri 2](https://tauri.app)** for desktop (Linux, Windows, macOS) and mobile (Android, later iOS).
- **Rust** for everything that touches data. Cryptography uses audited
  [RustCrypto](https://github.com/RustCrypto) crates; no home-grown primitives.
- **Svelte 5 + SvelteKit** (static SPA) for the interface. All assets are local; no CDN.

```
mossbook/
├── core/             mossbook-core: crypto/, store/, vault/, domain/, query/, journal.rs, crisis.rs
│   ├── data/crisis/  crisis phone numbers per country, with sources and check date
│   ├── migrations/   SQL schema
│   └── tests/        integration tests and the format-v1 fixture
├── app/
│   ├── src-tauri/    Tauri commands, session, capabilities
│   ├── src/          Svelte UI (routes/, lib/api, lib/i18n, lib/components)
│   └── e2e/          Playwright scenarios against a fake backend
├── docs/             crypto.md, backup-format.md, PLAN.md
├── tools/            verify-vectors.py, read-backup.py (independent readers)
└── SPEC.md           full specification (Polish)
```

## Building from source

### Prerequisites

- **Rust**: installed with [rustup](https://rustup.rs). The exact toolchain is pinned in
  [`rust-toolchain.toml`](rust-toolchain.toml) and installed automatically.
- **Node.js** 24 or newer, with npm.
- **Tauri CLI**: `cargo install tauri-cli --locked`
- **Platform libraries** for Tauri, see the
  [Tauri prerequisites](https://tauri.app/start/prerequisites/). On Debian/Ubuntu:

  ```sh
  sudo apt-get install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Run

```sh
git clone https://github.com/jakub-m-arch/mossbook.git
cd mossbook/app
npm ci
cargo tauri dev          # development build with hot reload
cargo tauri build        # release build and installers
```

The journal is stored as `vault.db` in the platform's application data directory,
for example `~/Library/Application Support/io.github.jakubmarch.mossbook/` on macOS
or `~/.local/share/io.github.jakubmarch.mossbook/` on Linux.

## Development

Run these before sending a change (CI runs the same):

```sh
# Rust (from the repository root)
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check         # licenses, advisories, banned networking crates
cargo audit

# Frontend (from app/)
npm run lint
npm run check
npm test
npx playwright install chromium   # once
npm run test:e2e
```

### Tests

- **Core**: round-trips, wrong passwords and recovery keys, every flipped byte, swapped
  records, tampered headers, property tests over arbitrary entries, and a committed
  vault file from format version 1 that every future build must still open.
- **App**: session state machine, auto-lock policy, and checks that the command list,
  the capability file and the TypeScript wrappers stay in sync.
- **UI**: unit tests (Vitest) and end-to-end scenarios (Playwright) against an
  in-browser stand-in for the Tauri backend.

All test data is synthetic.

## Contributing

Contributions are welcome, especially translations and reviews of the security design.

- Read the rules in [`CLAUDE.md`](CLAUDE.md); they apply to human contributors too:
  no network code, no plaintext on disk, no logging of user data, no home-grown crypto,
  no telemetry, and calm, non-clinical wording in the UI.
- Changes to `core/crypto`, `core/vault` or the file formats need tests, including
  negative ones. A format change means a version bump, a migration and a migration test.
- Sign off your commits (`git commit -s`) to certify the
  [Developer Certificate of Origin](https://developercertificate.org).
- Texts and their wording matter here. Interface copy speaks to the user in the second
  person, plainly and warmly, without diagnoses or judgement.

## License

Mossbook is free software under the [GNU General Public License v3.0 or later](LICENSE),
with an [additional permission](LICENSE-EXCEPTION.md) (GPLv3 section 7) that allows
distribution through app stores as long as the source code stays available to everyone.

Copyleft is deliberate: an app whose value is trust should not be forked into a closed
version with tracking added.

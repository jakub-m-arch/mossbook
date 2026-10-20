# Mossbook: rules for agents and contributors

Specification: `SPEC.md`. Plan, progress and decisions: `docs/PLAN.md` (update the
checklist after every stage). How the code fits together: `docs/architecture.md`.

## Non-negotiable
- NEVER add networking dependencies (reqwest, hyper, ureq, tauri-plugin-http, fetch to external addresses, CDNs). The app works 100% offline.
- NEVER write user data to disk in plaintext. Everything goes through `core::store` and encryption from `core::crypto`.
- NEVER log entry content, tags, passwords, keys or recovery phrases (`println!`, `log::*`, `tracing::*`, `console.log`). Domain types have a hand-written, redacting `Debug`.
- NEVER implement cryptographic primitives yourself. Use the RustCrypto crates listed in SPEC.md §6.6.
- Keep keys in `secrecy`/`zeroize`. The frontend never receives a key.
- No telemetry, analytics or external error reporting.
- UI texts: no diagnoses or clinical assessments, no gamification, nothing that makes the user feel guilty.

## Architecture
- Logic lives in `core/` (pure Rust, no Tauri). `app/src-tauri` is a thin command layer.
- Svelte 5 (runes) + TypeScript. The UI talks to Rust only through the typed wrappers in `app/src/lib/api/`.
- When in doubt, ask instead of guessing, especially about cryptography and file formats. Decisions made outside SPEC.md are in `docs/PLAN.md` §3.

## Quality
- Every change in `core/crypto`, `core/vault` or `core/backup` needs tests, including negative ones.
- Before finishing a task: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm run check` (in `app/`).
- Test data is synthetic only.
- Changing a record or backup format = version bump + migration + migration test.
- Code comments and documentation are written in English.

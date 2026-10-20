# Contributing to Mossbook

Thank you for helping. Mossbook is a journal for people who may be going through
hard times, so both the code and the words in it matter.

## Ground rules

These are not negotiable (see also [`CLAUDE.md`](CLAUDE.md)):

- **No network.** No networking crates or plugins, no fetches, no CDN assets.
- **No plaintext on disk.** All data goes through `core::store` and `core::crypto`.
- **No logging of user data** (`println!`, `log::*`, `tracing::*`, `console.*`).
- **No home-grown cryptography.** Only the RustCrypto crates listed in `SPEC.md` §6.6.
- **No telemetry**, analytics or remote error reporting.
- **Calm wording.** Second person, plain and warm; no diagnoses, judgement,
  gamification or guilt.

## Workflow

How the code is organised is described in [`docs/architecture.md`](docs/architecture.md).

1. Open an issue first for anything larger than a small fix.
2. Keep changes focused; add tests. Changes to `core/crypto`, `core/vault`,
   `core/backup` or a file format need negative tests too, and a format change
   needs a version bump, a migration and a fixture (see `docs/crypto.md`).
3. Run the checks from the README (`cargo fmt`, `clippy`, `test`, `deny`,
   `npm run lint`, `check`, `test`, `test:e2e`). CI runs the same.
4. Sign off every commit to certify the
   [Developer Certificate of Origin](https://developercertificate.org):
   `git commit -s`.

Test data must be synthetic. Never paste real journal content into issues, tests or
screenshots.

## Translations

UI texts live in `app/src/lib/i18n/{pl,en,uk}.json`. A test checks that every
language has the same keys and placeholders. Avoid sentences that need plural forms of
a number; rephrase as "Label: {count}". Native-speaker reviews of existing texts are
especially welcome.

## Crisis numbers

Numbers live in `core/data/crisis/{country}.json` with their sources and the date they
were checked. To add a country, include official sources (a government or the
operator's own page) and translations of the names and descriptions.

## License

By contributing you agree that your work is licensed under
[GPL-3.0-or-later](LICENSE) with the [app store permission](LICENSE-EXCEPTION.md).

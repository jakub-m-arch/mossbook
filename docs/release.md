# Release checklist

Releases are desktop builds published on GitHub Releases with `SHA256SUMS`
(SPEC §11). Mobile builds and app stores are postponed.

## Before tagging

- [ ] `main` is green in CI.
- [ ] **Crisis numbers re-checked** against the official sources listed in
      `core/data/crisis/*.json`; `verified_at` updated (SPEC §9).
- [ ] Independent checks pass:
      `tools/verify-vectors.py` and
      `tools/read-backup.py core/tests/fixtures/backup-v1.mossbook "fixture password v1"`.
- [ ] If any file format changed: version bumped, migration and new fixture added.
- [ ] Version bumped in `Cargo.toml` (`[workspace.package]`),
      `app/src-tauri/tauri.conf.json` and `app/package.json` (a test checks they match).
- [ ] `CHANGELOG.md` updated.
- [ ] Manual smoke test of a release build (`cd app && cargo tauri build`): create a
      journal, quick entry, lock/unlock, save and load a backup, crisis screen.

## Tagging

```sh
git tag -s v0.1.0 -m "Mossbook 0.1.0"   # or -a if you do not sign tags
git push origin v0.1.0
```

The `Release` workflow builds Linux, Windows and macOS (Apple Silicon and Intel)
installers and attaches them with `SHA256SUMS` to a **draft** release.

## Publishing

- [ ] Download one installer per platform from the draft and check it against
      `SHA256SUMS`.
- [ ] Review the release notes, then publish the draft. Publishing triggers the
      `Homebrew` workflow, which updates the cask in `jakub-m-arch/homebrew-mossbook`
      (needs the `HOMEBREW_TAP_TOKEN` secret; it is skipped with a notice otherwise).

Installers are not code-signed yet: macOS Gatekeeper and Windows SmartScreen will
warn on first launch (README, *Install*).

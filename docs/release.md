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
- [ ] `CHANGELOG.md` updated; `fastlane/metadata/android/*/changelogs/<versionCode>.txt`
      for the Android stores.
- [ ] Manual smoke test of a release build (`cd app && cargo tauri build`): create a
      journal, quick entry, lock/unlock, save and load a backup, crisis screen.
- [ ] PDF password: save a report with and without a password; open the encrypted
      file in Adobe Acrobat, macOS Preview/Quick Look, Chrome/Edge and Firefox; check
      that a wrong password fails, that Polish and Ukrainian text renders, and that the
      PDF metadata is not readable without the password. `tools/read-report.py` is an
      independent check (pypdf + cryptography).

## Tagging

```sh
git tag -s v0.1.0 -m "Mossbook 0.1.0"   # or -a if you do not sign tags
git push origin v0.1.0
```

Push the tag from the command line; do not create the release in the GitHub web
interface. A release made there is published at once, before any installer exists
(so the `Homebrew` workflow fails), and nothing checks that the version in the code
matches the tag.

The `Release` workflow builds Linux, Windows and macOS (Apple Silicon and Intel)
installers and attaches them with `SHA256SUMS` to a **draft** release.

## Publishing

- [ ] Download one installer per platform from the draft and check it against
      `SHA256SUMS`. For Android: install the APK over the previous version on a
      phone (the journal stays), check fingerprint unlock and that screenshots come
      out black.
- [ ] Review the release notes, then publish the draft. Publishing triggers the
      `Homebrew` workflow, which updates the cask in `jakub-m-arch/homebrew-mossbook`
      (needs the `HOMEBREW_TAP_TOKEN` secret; it is skipped with a notice otherwise).
      When that workflow finishes, the `Website` workflow rebuilds the site so the
      download links point at the new version.

Installers are not code-signed yet: macOS Gatekeeper and Windows SmartScreen will
warn on first launch (README, *Install*).

## Android signing key (once)

Android installs an update only if it is signed with the same key as the installed
app. **Losing the key means users must uninstall and restore from a backup**, so it is
generated once and kept safe.

```sh
"$(/usr/libexec/java_home -v 17)/bin/keytool" -genkeypair -v \
  -keystore ~/mossbook-release.jks -alias mossbook \
  -keyalg RSA -keysize 4096 -validity 36500 -dname "CN=Mossbook"
```

keytool asks for a password (PKCS12 uses the same one for the store and the key).
Then:

- [ ] Keep `mossbook-release.jks` and its password in a password manager or another
      encrypted, offline place, and nowhere in the repository (`*.jks` is ignored).
- [ ] Add three repository secrets (Settings → Secrets and variables → Actions):
      `ANDROID_KEYSTORE` = `base64 -i ~/mossbook-release.jks` (one line),
      `ANDROID_KEY_ALIAS` = `mossbook`, `ANDROID_KEY_PASSWORD` = the password.
- [ ] Note the certificate fingerprint for `SECURITY.md` and the stores:
      `keytool -list -v -keystore ~/mossbook-release.jks | grep SHA256`.

Without `ANDROID_KEYSTORE` the `Release` workflow skips the APK with a notice. With
it, it builds a signed APK (arm64, armv7, x86_64), runs
`tools/check-android-apk.sh` and `apksigner verify`, and uploads
`Mossbook_<version>_android.apk` before `SHA256SUMS` is computed. Locally, a
`keystore.properties` in `app/src-tauri/gen/android` (`storeFile`, `keyAlias`,
`password`; ignored by git) signs `cargo tauri android build --apk`.

The website shows the Android download as soon as the newest published release has
an APK (`site/render.sh … android`).

## Android stores

Descriptions, screenshots and changelogs are in `fastlane/metadata/android`
(en-US, pl-PL), the layout F-Droid and IzzyOnDroid read. Add
`changelogs/<versionCode>.txt` for every release; the version code is
`major * 1000000 + minor * 1000 + patch` (0.2.0 → 2000).

- **IzzyOnDroid** takes the signed APK from GitHub Releases, so it has the same
  signature as the APK on the website.
- **F-Droid** builds from source and signs with its own key unless the build is
  reproducible; then it can publish the APK with this signature. Until then the
  F-Droid build could not update a GitHub install, or the other way round.

## Updating GitHub Actions

Actions are pinned to commit SHAs. Dependabot proposes minor and patch updates
monthly; ordinary CI on its pull request covers `ci.yml`. Major versions are
ignored there and bumped by hand:

- [ ] Read the action's changelog for breaking changes (inputs, outputs,
      required permissions, Node runtime).
- [ ] Pin the new commit SHA with the version in a comment, as the other actions.
- [ ] If the action is used by `release.yml` or `homebrew.yml`, check it with a
      test tag (for example `v0.1.2-rc.1`), then delete the draft release and the tag.

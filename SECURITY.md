# Security policy

Mossbook stores some of the most private things a person writes. Security reports are
welcome and taken seriously.

## Reporting a vulnerability

Please **do not open a public issue**. Report privately through GitHub:
**Security → Report a vulnerability** on
[the repository](https://github.com/jakub-m-arch/mossbook/security/advisories/new).

Useful details: the version or commit, the platform, steps to reproduce and what an
attacker gains. Please use synthetic data only; never send real journal content.

You can expect an acknowledgement within a week. Fixes are released as soon as
possible and credited in the release notes, unless you prefer to stay anonymous.

## Supported versions

Only the latest release receives security fixes while the project is pre-1.0.

## Scope

In scope: the cryptography and file formats, anything that could expose journal
content or dates, bypasses of the lock, network access of any kind, and the commands
exposed to the web view.

What Mossbook protects against, and what it does not, is described in
[`docs/threat-model.md`](docs/threat-model.md). Known limitations listed there (for
example web view memory or unsigned installers) are not vulnerabilities by themselves,
but ideas to reduce them are very welcome.

## Verifying a release

Each release ships `SHA256SUMS`. Check a download before installing:

```sh
sha256sum --check --ignore-missing SHA256SUMS      # Linux
shasum -a 256 --check --ignore-missing SHA256SUMS  # macOS
```

Each installer also has a **build provenance attestation**, signed by GitHub for the
`Release` workflow run that built it. It shows which commit and workflow produced the
file, so a download cannot be swapped for one built elsewhere:

```sh
gh attestation verify Mossbook_0.1.2_aarch64.dmg --repo jakub-m-arch/mossbook
```

(Releases from 0.1.2 on.) The workflows pin every third-party GitHub Action to a commit
SHA, so a moved tag cannot change what the release build runs.

The Android APK (from 0.2.0) is signed with the Mossbook release key. Its
certificate SHA-256 fingerprint is:

```
D6:E7:DB:BD:82:39:09:2D:C3:81:26:64:43:10:9C:F6:47:82:3C:1C:00:05:4D:EE:1E:13:BB:46:AE:E5:68:B2
```

Check a download with `apksigner verify --print-certs Mossbook_0.2.0_android.apk`
(Android SDK build-tools), or compare it in Obtainium or AppVerifier. Android installs
an update only when it carries the same signature, so a forged APK cannot replace an
installed Mossbook.

The cryptographic design can be checked independently: `tools/verify-vectors.py`
recomputes the test vectors with libsodium and argon2-cffi, and `tools/read-backup.py`
opens a backup without any Mossbook code (see `docs/crypto.md`).

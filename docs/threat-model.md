# Threat model

Mossbook keeps a private journal on one device. This document lists what it defends
against, how, where that lives in the code, and what it deliberately does not cover.
It complements [`crypto.md`](crypto.md) (cryptography) and
[`backup-format.md`](backup-format.md) (backups); the requirements are in `SPEC.md` §2.

## Assets

- The content of entries, tags, the safety plan and settings.
- When things happened (dates and times are as sensitive as the text).
- The password and the 24-word recovery key.

## In scope

| Threat | Example | Mitigation | Where |
|---|---|---|---|
| Someone picks up the unlocked device | A housemate opens the app | Separate app password; lock on minimize/background (default) or after 1/5/15 min idle; lock button on every screen; lock on app exit | `app/src-tauri/src/session.rs`, `lib.rs` |
| Over-the-shoulder on desktop | Switching windows | Content is covered while the window is not focused | `privacy-curtain` event, `routes/+layout.svelte` |
| Password guessing in the app | Repeated attempts | Delay 1, 2, 4… 60 s after failures; Argon2id makes each guess slow | `core/src/vault/throttle.rs`, `crypto/kdf.rs` |
| Access to the files | Theft, repair shop, disk image, cloud copy of the data folder | Every record and setting encrypted with XChaCha20-Poly1305 under a random DEK; the DEK wrapped under Argon2id(password) and HKDF(recovery key) | `core/src/crypto/`, `core/src/vault/` |
| Offline brute force of a copied file | Attacker runs a GPU cracker | Argon2id, 64 MiB, `t` calibrated to ~0.75 s; strength meter at setup; 8-character minimum | `crypto/kdf.rs`, `app/src/lib/password.ts` |
| Tampering with the file | Swapping, editing or moving records | AEAD with AAD bound to vault id, record id and type; tampered records are isolated, not trusted | `crypto/aead.rs`, `vault/unlock.rs` |
| Tampering with the header | Hostile KDF parameters to exhaust memory | Parameters validated against upper bounds before use | `crypto/kdf.rs` |
| Tampering with a backup | Removing or reordering records | DEK-sealed SHA-256 manifest over all records | `core/src/backup/` |
| Plaintext dates on disk | Row keys revealing when entries were written | Random v4 ids (decision D-13); no date columns | `domain/mod.rs`, `core/migrations/` |
| Forgotten password | Loss of access | Recovery key, confirmed by retyping 3 words before anything is written | `vault/create.rs`, `routes/setup/` |
| Network exfiltration | A dependency phones home | No networking code; networking crates banned by `cargo deny`; strict CSP; no HTTP plugin | `deny.toml`, `tauri.conf.json` |
| Compromised web view calling commands | Injected script | Only the app's own commands are granted; keys never reach the web view; dialing limited to crisis numbers and saved plan contacts | `capabilities/default.json`, `commands.rs` |
| Content in logs | Crash or debug output | Logging macros forbidden by lints; redacted `Debug`; errors carry codes, not data | `clippy.toml`, `domain/`, `error.rs` |
| Quick entry while locked | Writing down a moment without the password | Sealed box to the vault's public key: entries can be added but nothing can be read; validated on unlock | `crypto/sealed.rs`, `journal.rs` |
| Clipboard | A copied sentence lingers | Cleared after 60 s if still unchanged | `commands.rs` (`clipboard_clear_if`) |
| Memory after locking | Content left in RAM | Keys in `SecretBox`; decrypted content and domain objects zeroed on drop | `crypto/mod.rs`, `ZeroizeOnDrop` in `domain/` |

## Accepted metadata leaks

Visible to someone with the vault file or a backup, by design:

- the number of records, and how many of each type (entry, tag, plan, settings);
- the number of quick entries waiting in the inbox until the next unlock;
- the approximate size of each record;
- the file's modification time;
- the KDF parameters and that the file is a Mossbook vault.

## Known limitations

- **Web view memory.** The password, recovery phrase and decrypted content shown on
  screen pass through JavaScript strings and the IPC layer, which cannot be wiped.
  They become unreachable after locking but may stay in memory until reused.
- **Swap and hibernation.** Memory is not locked (`mlock`), so the OS may write it to
  swap or a hibernation file while the journal is unlocked.
- **Deleting files.** Deleted vault files and SQLite free pages are not overwritten
  (pointless on flash storage); their content is ciphertext.
- **Screenshots on desktop.** Screen capture is not blocked on desktop; the content
  is covered only while the window is unfocused.
- **Unsigned installers.** Release binaries are not code-signed yet; verify them
  against `SHA256SUMS` from the release page.
- **Platform web view.** The UI runs in the system web view (WebKitGTK, WebView2,
  WKWebView); its security updates come from the operating system.
- **Old backups.** A backup opens with the password valid when it was made (D-9).
- **Inbox injection.** Someone with write access to the vault file can append
  quick entries (the public key is in the header). They can only become "to complete"
  entries of a known type with an in-range value, which the user sees and can delete.
- **Throttling** lives in memory: restarting the app resets the delay. The real
  protection against guessing is the cost of Argon2id.

## Out of scope

- A device with malware, root/administrator compromise or a keylogger.
- Physical coercion ("show me your phone").
- Memory analysis while the journal is unlocked.
- Android and iOS builds are not released yet; their platform protections
  (`FLAG_SECURE`, backup exclusion, no `INTERNET` permission) are planned, not shipped.

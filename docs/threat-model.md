# Threat model

Mossbook keeps a private journal on one device. This document lists what it defends
against, how, where that lives in the code, and what it deliberately does not cover.
It complements [`crypto.md`](crypto.md) (cryptography) and
[`backup-format.md`](backup-format.md) (backups); the requirements are in `SPEC.md` §2.

## Assets

- The content of entries, tags, the safety plan and settings.
- Voice notes: recordings of the user's voice (E-18), which identify the speaker
  even without their content.
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
| Losing the journal file | Disk error, a crash mid-write, deleting something by mistake | Automatic encrypted backups on lock (daily, newest seven, D-16); a reminder in the settings of when a backup was last saved elsewhere; a way to check the written-down recovery key | `core/src/backup/auto.rs`, `routes/settings/` |
| Memory after locking | Content left in RAM | Keys in `SecretBox`; decrypted content and domain objects zeroed on drop | `crypto/mod.rs`, `ZeroizeOnDrop` in `domain/` |
| Voice notes outside the journal | A plaintext copy in the gallery, a temporary file, a cloud-synced folder | Recorded only inside the app (no import); encoded in memory and stored encrypted like records; decrypted one at a time for playback into an object URL that is revoked when the player goes away; never written to a file; the archive has no metadata or timestamps; recordings nothing points to are deleted on lock | `app/src/lib/voice/`, `journal.rs`, `vault/mod.rs` |
| Sync between devices (D-22) | The sync folder is copied by Syncthing, read by another app on the phone, or a file in it is replaced | Each file is a full encrypted backup under the journal's key: unreadable without it, and a changed, cut or foreign file is refused (manifest over records, recordings and the password wrap; vault id checked). An old file replayed cannot undo a newer edit, a deletion or a password change (newer `updated_at` and `password_epoch` win). Mossbook never connects; Syncthing does, between the user's own devices | `core/src/journal/sync.rs`, `backup/`, `app/src-tauri/src/sync.rs` |
| Joining another device's journal | Someone puts their own journal file into the folder | Joining needs that journal's password, is throttled like unlocking, warns that the password and recovery key change, and keeps this device's journal in its automatic backups first | `session.rs` (`sync_join`), Settings → Sync |
| A web view asking for more | Script asking for the camera or location | The web view is allowed the microphone (voice notes) and refused every other permission; Android declares only `RECORD_AUDIO`, asked on the first recording | `app/src-tauri/src/lib.rs`, `AndroidManifest.xml` |

## Accepted metadata leaks

Visible to someone with the vault file or a backup, by design:

- the number of records, and how many of each type (entry, tag, plan, settings);
- the number of voice notes and the size of each, which hints at its length;
- with sync on: in `meta`, this device's random id and the location of the sync
  folder; in the folder, how many devices there are, the size of each one's file and
  when it changed (when that device last locked with something new);
- the number of quick entries waiting in the inbox until the next unlock;
- the approximate size of each record;
- the file's modification time, and those of the automatic backups, which show on
  which of the last few days the journal was locked (at most one per day);
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
  Automatic backups are replaced after a password change; backups saved elsewhere are
  the user's to delete.
- **Plain exports.** The readable copy (a ZIP with the JSON and the voice notes) is
  not encrypted, and the recordings in it are the user's voice. The PDF report is not
  encrypted by default; it can optionally be protected with a password, but that is
  a property of the file, not of the relationship with the therapist. The app warns
  before writing either file; what happens to them afterwards is outside its control.
- **Inbox injection.** Someone with write access to the vault file can append
  quick entries (the public key is in the header). They can only become "to complete"
  entries of a known type with an in-range value, which the user sees and can delete.
- **Fingerprint unlock (Android, D-18).** When it is on, anyone who can make the
  phone accept a fingerprint opens the journal: a finger pressed on the sensor
  under coercion, or a fingerprint someone added to the phone. Adding a fingerprint
  needs the phone's screen lock and destroys Mossbook's key, so the second case asks
  for the journal password again. Mossbook does not throttle fingerprint attempts;
  Android does. The device key passes through the app's JVM memory as a string that
  cannot be wiped. Turning it off in Settings removes the key file and the Keystore
  key.
- **Voice notes in memory.** While a recording is made or played, its audio passes
  through the web view (the microphone stream, the encoder, the decoded bytes),
  which cannot be wiped, like typed text. The operating system shows that the
  microphone is in use; Mossbook does not record in the background (leaving the app
  ends the recording).
- **Sync and clocks.** Which of two edits is newer comes from each device's clock: a
  device whose clock is far ahead can make its older edit win. A sync file holds the
  whole journal, so anyone who can open the folder can copy it, though not read it;
  removing the folder from Syncthing on a lost device is up to the user. Joining
  makes both devices open with the first one's password and recovery key; the
  second device's previous key stops working (its old journal stays in its
  automatic backups, which still open with its old password).
- **Throttling** lives in memory: restarting the app resets the delay. The real
  protection against guessing is the cost of Argon2id.

## PDF report password

An optional password can protect a report PDF with AES-256 (PDF 2.0, V5/R6). What it
does and does not cover:

| Situation | Protected? |
|---|---|
| The file is found in a download folder, on a USB stick or in a disk backup | Yes, if the password is strong |
| The email carrying the PDF is read without the password | Yes |
| The password is sent in the same email or chat as the PDF | No — the protection is then only apparent |
| The therapist's device, account or backups | No — the file is already open there |
| A screenshot, a printout or a forwarded decrypted copy | No |
| A weak password or the journal password reused for the PDF | No — PDF encryption does not use Argon2id |

The password is entered or generated in the UI, never stored and never logged. The
report form enforces a 12-character minimum and recommends a generated password; it
also warns against reusing the journal password, because the therapist learns this one. The
encryption covers the PDF metadata as well. The file name and the fact that a PDF was
created are not hidden.

The password is **not** a substitute for choosing what goes into the report, and it
does not replace the encrypted `.mossbook` backup.

## Out of scope

- A device with malware, root/administrator compromise or a keylogger.
- Physical coercion ("show me your phone").
- Memory analysis while the journal is unlocked.
- Android and iOS builds are not released yet; their platform protections
  (`FLAG_SECURE`, backup exclusion, no `INTERNET` permission) are planned, not shipped.

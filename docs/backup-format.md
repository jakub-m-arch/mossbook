# `.mossbook` backup format

A Mossbook backup is a single file holding the encrypted journal. This document is
normative: [`tools/read-backup.py`](../tools/read-backup.py) is an independent reader
written from it, and checks the committed fixtures
`core/tests/fixtures/backup-v1.mossbook`, `backup-v2.mossbook` and `backup-v3.mossbook`.
A sync file (decision D-22) is a backup in this format, written by one of the user's
devices for the others; see *Sync files* below.

Cryptographic primitives and key derivation are described in [`crypto.md`](crypto.md).

## Layout

```
offset  size  content
0       8     magic "MOSSBOOK" (ASCII)
8       2     backup format version, u16 little-endian (currently 3; 1 and 2 are still read)
10      …     body: one CBOR item (RFC 8949)
```

Files larger than 512 MiB are rejected before parsing.

## Body

A CBOR map with these keys (byte strings are CBOR major type 2):

| key               | type                 | content                                                     |
|-------------------|----------------------|-------------------------------------------------------------|
| `vault_format`    | unsigned int         | `format_version` of the vault: equal to the backup version  |
| `vault_id`        | bytes (16)           | vault identifier                                            |
| `kdf_params`      | bytes (13)           | Argon2id parameters, encoding in `crypto.md`                |
| `kdf_salt`        | bytes (16)           | password salt                                               |
| `wrapped_dek_pwd` | bytes (72)           | DEK wrapped under the password key                          |
| `wrapped_dek_rec` | bytes (72)           | DEK wrapped under the recovery key                          |
| `password_epoch`  | unsigned int         | version 3: how many times the password was changed (0 if absent) |
| `records`         | array of maps        | `{ "id": bytes(16), "type": uint 1–4 or 6, "blob": bytes }` (6, a tombstone, from version 3) |
| `media`           | array of maps        | version 2 only, may be absent when empty: recordings, `{ "id": bytes(16), "type": 5, "blob": bytes }` |
| `manifest`        | bytes                | sealed digest of the records (and recordings), see below    |

The header fields, record blobs and recording blobs are copied unchanged from
`vault.db`: each blob is `nonce || ciphertext || tag` with the record AAD from
`crypto.md`, and its plaintext is the CBOR record described in *Record contents*
there, or for `media` an Ogg Opus file (voice note, E-18). `records` never holds
type 5 and `media` only type 5; anything else rejects the file.

Version 1 (Mossbook 0.1–0.3) has no `media` key and `vault_format` 1; version 2
(voice notes) has no tombstones and no `password_epoch`. In every version
`vault_format` equals the backup version.

## Manifest

Records are authenticated one by one by their AAD. The manifest additionally binds the
**set and order** of records, so removing, adding, duplicating or reordering records is
detected:

```
list(rows) = u64le(len(rows))
             || for each row in file order:
                  id (16) || type (1) || u64le(len(blob)) || blob

header   = u32le(password_epoch) || kdf_params (13) || kdf_salt (16)
           || wrapped_dek_pwd (72) || wrapped_dek_rec (72)

version 1:  digest = SHA-256( list(records) )
version 2:  digest = SHA-256( list(records) || list(media) )
version 3:  digest = SHA-256( header || list(records) || list(media) )
AAD      = "mossbook/backup/v" || version (ASCII) || vault_id

manifest = nonce (24) || XChaCha20-Poly1305(DEK, AAD, digest) || tag (16)
```

The version is part of the AAD, so a backup cannot be relabelled as an older
version with its recordings or tombstones cut out. From version 3 the header fields
another device may adopt (the password wrap and its count) are covered too, so a
sync file cannot be altered to roll a password back.

## Opening a backup

1. Check the magic, the version and the structure (field lengths, record types).
2. Derive the KEK from the password (Argon2id with the backup's `kdf_params` and
   `kdf_salt`) or from the recovery phrase (HKDF), and unwrap the DEK. Failure means a
   wrong password or recovery key.
3. Open the manifest with the DEK and compare it with the recomputed digest.
4. Decrypt every record and every recording (one at a time). Any failure in steps
   3–4 rejects the whole file as damaged.

A backup opens with the password (and recovery key) that were valid **when it was
made**. After a password change, older backups still need the old password
(decision D-9 in `PLAN.md`); the recovery key never changes.

## Bringing a backup in

- **Restore** (no journal on the device): the header and records are written as a new
  `vault.db` unchanged. The restored journal keeps the backup's identity, password and
  recovery key.
- **Import** into an existing, unlocked journal: records are decrypted with the
  backup's DEK and re-encrypted with the current vault's DEK and `vault_id`
  (decision D-4), in one transaction.
  - *Merge*: missing entries and tags are added; an entry present in both keeps the
    version with the later `updated_at`; a backup tag whose name matches an existing tag
    (case-insensitive) is mapped onto it; the current safety plan and settings stay.
  - *Replace*: the journal becomes exactly the backup's content.
  - Records that do not decode or fail validation are skipped and counted.

## Automatic backups

When the journal locks, at most once every 20 hours, the app writes an ordinary backup
(exactly the format above) into a `backups/` folder next to `vault.db`, and keeps the
newest seven (decision D-16, `core/src/backup/auto.rs`).

- File names are fixed slots, `auto-1.mossbook` … `auto-7.mossbook`, so no date
  appears in plaintext. A new backup goes into a free slot, or over the oldest one,
  through a temporary file and a rename.
- They open like any backup: with the password or recovery key valid when they were
  written. After a password change (or a recovery with a new password) they are
  replaced by one backup under the new password, so old copies stop opening with the
  old one. Deleting the journal (V-9) deletes them too.
- They can be turned off in the settings. They protect against a damaged vault file
  or a mistake, not against losing the device.

## Sync files

The user's devices that share one journal (same vault id and DEK, decision D-22)
each write the whole journal as a backup named `<device id>.mossbook-sync` into a
folder that Syncthing carries between them. Mossbook never connects anywhere; it
only reads and writes that folder. A device opens the others' files with the shared
DEK instead of the password (the vault id must match) and runs the same checks as
for any backup: manifest, every record and recording. It then merges them:

- per entry, tag, plan and settings, the newer `updated_at` wins; at the same time a
  tombstone beats an edit, and of two different edits the larger CBOR encoding wins,
  so every device ends up with the same content whatever order files arrive in;
- a tombstone (`{ id, of: "entry" | "tag", deleted_at }`, record type 6 at the
  deleted record's id) beats an older edit, and an edit made after it brings the
  record back;
- two tags with the same name become one: the smaller id stays;
- a higher `password_epoch` means a newer password: its wrap is adopted;
- the unsaved entry form (D-20) is never taken from another device.



Settings → *Readable copy (ZIP)* writes the whole journal, **unencrypted**, as a ZIP
archive (X-4, decision D-21), after a warning. It is meant for leaving Mossbook or
keeping one's own archive; the app does not import it. Entries are stored without
compression and without timestamps:

```
mossbook-export.json        the journal as JSON, below
voice/<voice id>.ogg        each voice note, Ogg Opus, named by `entries[].voice.id`
```

```json
{
  "format": "mossbook-export",
  "version": 2,
  "exported_at": "2026-10-20T11:00:00+02:00",
  "entries": [ … ],
  "tags": [ … ],
  "plan": { … },
  "settings": { … }
}
```

`settings` never contains `unsaved_entry` (a form kept across a lock, D-20): a
half-filled form is not part of the journal.

`entries` (oldest first), `tags` (by name), `plan` and `settings` have exactly the
shapes of the domain types in `core/src/domain/` (field names in `snake_case`,
timestamps in RFC 3339 with the local offset, ids as UUID strings, entry kinds as
`{"fear": {…}}` or `"note"`). An entry with a recording has
`"voice": { "id": …, "duration_ms": …, "mime": "audio/ogg; codecs=opus" }`.
A change to these shapes increments `version`: version 1 (Mossbook 0.1–0.3) was the
JSON file alone, without `voice`.

## Versioning

A change to anything above is a new backup format version, with a new fixture in
`core/tests/fixtures/` and a test that the previous versions still import
(see `CLAUDE.md`). Unknown versions are rejected, never guessed.

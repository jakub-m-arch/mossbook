# `.mossbook` backup format

A Mossbook backup is a single file holding the encrypted journal. This document is
normative: [`tools/read-backup.py`](../tools/read-backup.py) is an independent reader
written from it, and checks the committed fixture
`core/tests/fixtures/backup-v1.mossbook`.

Cryptographic primitives and key derivation are described in [`crypto.md`](crypto.md).

## Layout

```
offset  size  content
0       8     magic "MOSSBOOK" (ASCII)
8       2     backup format version, u16 little-endian (currently 1)
10      …     body: one CBOR item (RFC 8949)
```

Files larger than 512 MiB are rejected before parsing.

## Body (version 1)

A CBOR map with these keys (byte strings are CBOR major type 2):

| key               | type                 | content                                                     |
|-------------------|----------------------|-------------------------------------------------------------|
| `vault_format`    | unsigned int         | `format_version` of the vault the records come from (1)     |
| `vault_id`        | bytes (16)           | vault identifier                                            |
| `kdf_params`      | bytes (13)           | Argon2id parameters, encoding in `crypto.md`                |
| `kdf_salt`        | bytes (16)           | password salt                                               |
| `wrapped_dek_pwd` | bytes (72)           | DEK wrapped under the password key                          |
| `wrapped_dek_rec` | bytes (72)           | DEK wrapped under the recovery key                          |
| `records`         | array of maps        | `{ "id": bytes(16), "type": uint 1–4, "blob": bytes }`      |
| `manifest`        | bytes                | sealed digest of the records, see below                     |

The header fields and record blobs are copied unchanged from `vault.db`: each blob
is `nonce || ciphertext || tag` with the record AAD from `crypto.md`, and its
plaintext is the CBOR record described in *Record contents* there.

## Manifest

Records are authenticated one by one by their AAD. The manifest additionally binds the
**set and order** of records, so removing, adding, duplicating or reordering records is
detected:

```
digest   = SHA-256( u64le(n)
                    || for each record in file order:
                         id (16) || type (1) || u64le(len(blob)) || blob )
manifest = nonce (24) || XChaCha20-Poly1305(DEK, AAD, digest) || tag (16)
AAD      = "mossbook/backup/v1" || vault_id
```

## Opening a backup

1. Check the magic, the version and the structure (field lengths, record types).
2. Derive the KEK from the password (Argon2id with the backup's `kdf_params` and
   `kdf_salt`) or from the recovery phrase (HKDF), and unwrap the DEK. Failure means a
   wrong password or recovery key.
3. Open the manifest with the DEK and compare it with the recomputed digest.
4. Decrypt every record. Any failure in steps 3–4 rejects the whole file as damaged.

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

## Versioning

A change to anything above is a new backup format version, with a new fixture in
`core/tests/fixtures/` and a test that the previous versions still import
(see `CLAUDE.md`). Unknown versions are rejected, never guessed.

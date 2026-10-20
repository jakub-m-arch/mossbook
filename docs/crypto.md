# Mossbook cryptography

This document describes the cryptographic design implemented in `core/src/crypto/`.
It is normative: an independent tool built from it must be able to read Mossbook data.
The rationale and threat model are in `SPEC.md` §2 and §6.

All primitives come from RustCrypto crates (`chacha20poly1305`, `argon2`, `hkdf`, `sha2`)
and `bip39`. Mossbook implements no primitives of its own.

## Key hierarchy

```
password ──NFC──Argon2id(salt, params)──▶ KEK_pwd ──┐
                                                    ├─ wrap ─▶ DEK (random, 256 bit) ──▶ records
recovery key (256 bit) ──HKDF-SHA256──▶ KEK_rec ────┘
```

- **DEK** is generated once with the OS CSPRNG and never changes. A password change
  re-wraps the DEK under a new `KEK_pwd` with a new salt; records are not re-encrypted.
- The DEK is wrapped twice, once under each KEK. Either one unlocks the vault.

## Primitives and encodings

### AEAD

XChaCha20-Poly1305 (24-byte nonce, 16-byte tag). Every ciphertext ("blob") is

```
nonce (24 B, random) || ciphertext || tag (16 B)
```

Nonces are always random (192 bits, so collisions are negligible); there are no counters.

### Records

```
AAD = "mossbook/rec/v1" || vault_id (16 B) || record_id (16 B) || record_type
```

`record_type` is one of the ASCII strings `entry`, `tag`, `plan`, `settings`, stored in the
`records.type` column as `1`, `2`, `3`, `4`. Binding the id, type and vault into the AAD
means a blob moved to another row, given another type, or copied into another vault
fails authentication.

### Key wrap

```
AAD     = "mossbook/wrap/v1" || vault_id (16 B)
wrapped = AEAD(KEK, AAD, DEK)        -- 24 + 32 + 16 = 72 bytes
```

### Password KDF

- Argon2id, version 0x13, 32-byte output, 16-byte random salt.
- The password is normalized to Unicode **NFC** before hashing, so precomposed and
  combining diacritics (e.g. `ż` vs `z` + U+0307) give the same key on every platform.
  No other transformation is applied (no trimming, no case folding).
- Default parameters: `m = 65536 KiB (64 MiB), t = 3, p = 1`. When a vault is created,
  `t` is raised (never lowered, at most 16) until one derivation takes about 0.75 s on
  the device. `m` and `p` stay at the defaults.
- Parameter encoding in the vault header (13 bytes):

  ```
  alg (1 B, 0x01 = Argon2id v0x13) || m_kib (u32 LE) || t (u32 LE) || p (u32 LE)
  ```

- When reading a header, parameters are rejected if `p ∉ 1..=8`, `t ∉ 1..=64`,
  `m_kib > 1 GiB` or `m_kib < 8·p`. This caps the work a tampered header can force.

### Recovery key

- 32 random bytes, shown as a 24-word BIP-39 phrase (English word list, standard checksum).
- Input is normalized before parsing: split on any whitespace, lowercased, joined with
  single spaces. Exactly 24 words are required; the checksum must be valid.
- `KEK_rec = HKDF-SHA256(ikm = entropy, salt = none, info = "mossbook/recovery/v1", L = 32)`.
  The entropy is uniformly random, so no salt is needed.
- During setup, the user must retype 3 randomly chosen words before the vault is
  written to disk (decision D-3 in `docs/PLAN.md`).

## Vault file

`vault.db` is an SQLite database (schema in `core/migrations/`, version tracked in
`PRAGMA user_version`; a file from a newer build is refused, never modified).
Pragmas: `journal_mode = WAL`, `secure_delete = ON`, `synchronous = FULL`; the WAL is
checkpointed and truncated when the vault is locked.

The header lives in the `meta` table; every value is random, public or ciphertext:

| key               | value                                              |
|-------------------|----------------------------------------------------|
| `format_version`  | `u16` LE, currently `1`                             |
| `vault_id`        | 16 random bytes (UUID v4 layout)                    |
| `kdf_params`      | 13 bytes, see *Password KDF*                        |
| `kdf_salt`        | 16 random bytes; replaced on every password change  |
| `wrapped_dek_pwd` | 72 bytes, DEK wrapped under `KEK_pwd`              |
| `wrapped_dek_rec` | 72 bytes, DEK wrapped under `KEK_rec`              |

Records are rows `(id BLOB(16), type INTEGER, blob BLOB)` in `records`, see *Records*.

Behaviour that matters for independent readers and for the threat model:

- **Creation** happens in memory; the file is written only after the user retyped 3
  recovery words, first as `vault.db.creating` and then renamed to `vault.db`.
- **Unlocking** decrypts every record into memory. A record that fails authentication
  or has an unknown type is skipped and counted (`unreadable_records`), so one damaged
  row does not lock the user out of the rest.
- **Tampering with the header** fails closed: a changed `vault_id`, salt or KDF params
  makes the unwrap fail (reported as a wrong password); a missing key or a hostile
  parameter value is reported as a damaged file.
- **Password change and recovery** write a new salt and `wrapped_dek_pwd` in one
  transaction. The recovery key never changes.
- **Failed attempts** are delayed 1 s, 2 s, 4 s … up to 60 s, in memory only
  (decision D-5). Data is never wiped automatically.
- **Destroying** the vault removes `vault.db`, `-wal` and `-shm` after re-checking the
  password. The files are not overwritten, since that does not help on flash storage
  and the content is ciphertext anyway.

## Record contents

The plaintext of every record is CBOR (RFC 8949), produced by `serde` + `ciborium` from the
types in `core/src/domain/`:

| `records.type` | content                         | row id                                   |
|----------------|---------------------------------|------------------------------------------|
| 1 `entry`      | `Entry` (with `schema` = 1)     | the entry's UUID, equal to its `id`      |
| 2 `tag`        | `Tag`                           | the tag's UUID, equal to its `id`        |
| 3 `plan`       | `SafetyPlan`                    | fixed `00000000-0000-0000-0000-000000000003` |
| 4 `settings`   | `Settings`                      | fixed `00000000-0000-0000-0000-000000000004` |

- Structs are CBOR maps keyed by snake_case field names; enums use serde's external
  tagging with snake_case variant names (`{"fear": {...}}`, `"note"`).
- Ids are random UUIDs (v4) and are the only plain-text value per record, so they
  must not carry time: UUIDv7, as SPEC §5 suggests, would reveal when each entry was
  written (decision D-13). Some early pre-release vaults contain v7 ids; they are
  read normally. Ids are 16-byte CBOR byte strings. (Over IPC to the UI, which uses JSON,
  they are hyphenated UUID strings.)
- Timestamps are RFC 3339 strings with the original UTC offset
  (`2026-10-19T21:30:00+02:00`), so the local time of an entry survives travel.
- Missing optional fields decode to their defaults; unknown fields are ignored.
  Adding a field with a default is therefore not a format change.
- A record whose decoded `id` differs from its row id, or that does not decode, is
  counted as unreadable on unlock, like a record that fails authentication.

Domain types that hold user text are zeroed on drop (`ZeroizeOnDrop`) and have a
redacted `Debug`, so content is wiped when the vault locks and never reaches logs.

## Secrets in memory

- DEK, KEKs and recovery entropy live in `secrecy::SecretBox` and are zeroed on drop.
  Intermediate buffers (Argon2 output, unwrapped key bytes, normalized password and
  phrase, decrypted plaintext) are zeroed as well.
- Key types have no `Clone` and a redacted `Debug` (`Dek([REDACTED])`).
- Errors (`CryptoError`) never carry input data.
- The webview never receives key material. Limits of the JS side (password strings
  that cannot be wiped) are documented in `SECURITY.md`.

## Test vectors

Each module has a fixed vector test (`fixed_vector`). The values were cross-checked
against libsodium (PyNaCl), argon2-cffi and a stdlib HKDF:

```sh
python3 -m venv .venv && .venv/bin/pip install pynacl argon2-cffi
.venv/bin/python tools/verify-vectors.py
```

A change in any vector means the on-disk format changed: bump the format version,
add a migration and a migration test (see `CLAUDE.md`).

`core/tests/fixtures/vault-v1.db` is a vault written by format version 1 (password
`fixture password v1`, synthetic records). The test `opens_format_v1_fixture` must keep
passing in every later build; add a new fixture for every new format version.

## Inbox: quick entries while locked (E-15)

The vault has an X25519 key pair for appending entries without the password:

| key                   | value                                                          |
|-----------------------|----------------------------------------------------------------|
| `x25519_pub`          | 32 bytes, public key                                           |
| `wrapped_x25519_priv` | 72 bytes, `AEAD(DEK, "mossbook/inbox-key/v1" ‖ vault_id, secret)` |

- A quick entry from the lock screen is the CBOR of `InboxEntry { kind, value,
  occurred_at }`, sealed with libsodium's `crypto_box_seal` (ephemeral X25519 +
  XSalsa20-Poly1305) to `x25519_pub` and stored in `inbox(id, blob)`.
  Interoperability is tested against PyNaCl.
- On unlock the secret key is unwrapped with the DEK, each item is opened and
  validated, stored as a regular encrypted entry under its inbox id ("to complete"),
  and the inbox is cleared. Items that do not open or validate are counted as
  unreadable and never trusted.
- The payload is deliberately minimal: anyone with the file can append, so an
  injected item can only become a "to complete" entry of a known type with an
  in-range value.
- Vaults created before the inbox existed get a key pair on their next unlock.
  Backups do not carry the pair; a restored vault gets a new one.

## Backups

The `.mossbook` backup format is specified in [`backup-format.md`](backup-format.md).

## Not covered yet

- Biometric unlock (v1, SPEC §6.5).

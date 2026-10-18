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

## Not covered yet

- Vault header layout in SQLite `meta` (stage 3) and the `.mossbook` backup format
  (stage 9, `docs/backup-format.md`).
- Sealed-box inbox for quick entries without unlocking and biometric unlock (v1, SPEC §6.4–6.5).

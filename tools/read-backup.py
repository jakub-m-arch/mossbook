#!/usr/bin/env python3
"""Independent reader for `.mossbook` backups, written from docs/backup-format.md.

Opens a backup (format version 1, 2, 3 or 4) with its password, verifies the manifest,
every record and every recording, and prints a summary (no content unless --show
is given). With --voice DIR, recordings are written there as .ogg files.

    python3 -m venv .venv && .venv/bin/pip install pynacl argon2-cffi cbor2
    .venv/bin/python tools/read-backup.py core/tests/fixtures/backup-v4.mossbook "fixture password v4"
"""
import argparse
import hashlib
import struct
import sys
import unicodedata

import cbor2
from argon2.low_level import Type, hash_secret_raw
from nacl.bindings import crypto_aead_xchacha20poly1305_ietf_decrypt as xchacha_decrypt

TYPES = {1: "entry", 2: "tag", 3: "plan", 4: "settings", 6: "tombstone"}
MEDIA_TYPES = {5: "voice"}


def open_blob(key: bytes, aad: bytes, blob: bytes) -> bytes:
    return xchacha_decrypt(blob[24:], aad, blob[:24], key)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("file")
    parser.add_argument("password")
    parser.add_argument("--show", action="store_true", help="print decrypted records")
    parser.add_argument("--voice", metavar="DIR", help="write decrypted recordings here")
    args = parser.parse_args()

    data = open(args.file, "rb").read()
    if data[:8] != b"MOSSBOOK":
        sys.exit("not a Mossbook backup")
    (version,) = struct.unpack("<H", data[8:10])
    if version not in (1, 2, 3, 4):
        sys.exit(f"unsupported backup version {version}")
    body = cbor2.loads(data[10:])
    # Backup version N comes from vault format N.
    if body["vault_format"] != version:
        sys.exit("unsupported vault format")
    media = body.get("media", [])
    if version == 1 and media:
        sys.exit("a version 1 backup holds no recordings")
    if version < 3 and any(r["type"] == 6 for r in body["records"]):
        sys.exit("tombstones exist from version 3")

    vault_id = body["vault_id"]
    alg, m_kib, t, p = struct.unpack("<BIII", body["kdf_params"])
    if alg != 1:
        sys.exit("unknown KDF")
    password = unicodedata.normalize("NFC", args.password).encode()
    kek = hash_secret_raw(password, body["kdf_salt"], time_cost=t, memory_cost=m_kib,
                          parallelism=p, hash_len=32, type=Type.ID, version=19)
    try:
        dek = open_blob(kek, b"mossbook/wrap/v1" + vault_id, body["wrapped_dek_pwd"])
    except Exception:
        sys.exit("wrong password")

    digest = hashlib.sha256()
    if version >= 3:
        # The password wrap and its change count, which other devices adopt (sync).
        digest.update(struct.pack("<I", body.get("password_epoch", 0)))
        digest.update(body["kdf_params"] + body["kdf_salt"])
        digest.update(body["wrapped_dek_pwd"] + body["wrapped_dek_rec"])
    lists = [body["records"]] + ([media] if version >= 2 else [])
    for rows in lists:
        digest.update(struct.pack("<Q", len(rows)))
        for r in rows:
            digest.update(r["id"] + bytes([r["type"]]) + struct.pack("<Q", len(r["blob"])) + r["blob"])
    label = f"mossbook/backup/v{version}".encode()
    if open_blob(dek, label + vault_id, body["manifest"]) != digest.digest():
        sys.exit("manifest mismatch: records or recordings were changed, added or removed")

    counts: dict[str, int] = {}
    for r in body["records"]:
        kind = TYPES[r["type"]]
        aad = b"mossbook/rec/v1" + vault_id + r["id"] + kind.encode()
        record = cbor2.loads(open_blob(dek, aad, r["blob"]))
        counts[kind] = counts.get(kind, 0) + 1
        if args.show:
            print(kind, record)
    for r in media:
        kind = MEDIA_TYPES[r["type"]]
        aad = b"mossbook/rec/v1" + vault_id + r["id"] + kind.encode()
        recording = open_blob(dek, aad, r["blob"])
        if not recording.startswith(b"OggS"):
            sys.exit("a recording is not an Ogg file")
        counts[kind] = counts.get(kind, 0) + 1
        if args.voice:
            name = r["id"].hex() + ".ogg"
            with open(f"{args.voice}/{name}", "wb") as f:
                f.write(recording)
    print("ok:", ", ".join(f"{n} {k}" for k, n in sorted(counts.items())))
    return 0


if __name__ == "__main__":
    sys.exit(main())

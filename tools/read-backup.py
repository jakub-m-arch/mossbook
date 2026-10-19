#!/usr/bin/env python3
"""Independent reader for `.mossbook` backups, written from docs/backup-format.md.

Opens a backup with its password, verifies the manifest and every record, and
prints a summary (no content unless --show is given).

    python3 -m venv .venv && .venv/bin/pip install pynacl argon2-cffi cbor2
    .venv/bin/python tools/read-backup.py core/tests/fixtures/backup-v1.mossbook "fixture password v1"
"""
import argparse
import hashlib
import struct
import sys
import unicodedata

import cbor2
from argon2.low_level import Type, hash_secret_raw
from nacl.bindings import crypto_aead_xchacha20poly1305_ietf_decrypt as xchacha_decrypt

TYPES = {1: "entry", 2: "tag", 3: "plan", 4: "settings"}


def open_blob(key: bytes, aad: bytes, blob: bytes) -> bytes:
    return xchacha_decrypt(blob[24:], aad, blob[:24], key)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("file")
    parser.add_argument("password")
    parser.add_argument("--show", action="store_true", help="print decrypted records")
    args = parser.parse_args()

    data = open(args.file, "rb").read()
    if data[:8] != b"MOSSBOOK":
        sys.exit("not a Mossbook backup")
    (version,) = struct.unpack("<H", data[8:10])
    if version != 1:
        sys.exit(f"unsupported backup version {version}")
    body = cbor2.loads(data[10:])
    if body["vault_format"] != 1:
        sys.exit("unsupported vault format")

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

    digest = hashlib.sha256(struct.pack("<Q", len(body["records"])))
    for r in body["records"]:
        digest.update(r["id"] + bytes([r["type"]]) + struct.pack("<Q", len(r["blob"])) + r["blob"])
    if open_blob(dek, b"mossbook/backup/v1" + vault_id, body["manifest"]) != digest.digest():
        sys.exit("manifest mismatch: records were changed, added or removed")

    counts: dict[str, int] = {}
    for r in body["records"]:
        kind = TYPES[r["type"]]
        aad = b"mossbook/rec/v1" + vault_id + r["id"] + kind.encode()
        record = cbor2.loads(open_blob(dek, aad, r["blob"]))
        counts[kind] = counts.get(kind, 0) + 1
        if args.show:
            print(kind, record)
    print("ok:", ", ".join(f"{n} {k}" for k, n in sorted(counts.items())))
    return 0


if __name__ == "__main__":
    sys.exit(main())

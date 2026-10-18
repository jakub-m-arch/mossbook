#!/usr/bin/env python3
"""Independently recompute the fixed crypto vectors of mossbook-core.

Uses libsodium (PyNaCl), argon2-cffi and a stdlib HKDF, so the vectors in
core/src/crypto/*.rs are checked against implementations other than RustCrypto.

    python3 -m venv .venv && .venv/bin/pip install pynacl argon2-cffi
    .venv/bin/python tools/verify-vectors.py
"""
import hashlib
import hmac
import sys
import unicodedata

from argon2.low_level import Type, hash_secret_raw
from nacl.bindings import crypto_aead_xchacha20poly1305_ietf_encrypt as xchacha_encrypt

VAULT = b"\x11" * 16
REC_A = b"\xaa" * 16

EXPECTED = {
    "aead": "000102030405060708090a0b0c0d0e0f1011121314151617e193f794a5cea8abd212c135761888dbb2ad64c8086ae00233da7c958f554aecd9ae8e2f",
    "kdf": "42f38f1d97f523ad2170abeddfcd7c24fd8370c9b3c6408653337c0fd93896f6",
    "recovery": "a3f3d01aa17e8f009d0eee4c9d5f7293c9d4ad20b01e42b5c5e121453ff91968",
    "wrap": "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeffe0e1e2e3e4e5e6e768707b08629c217797673da14b330b9c0d294353112bcebeccfce6c742cb59561ccc9ba21acb4137910f3c513ee5177b",
}


def hkdf_sha256(ikm: bytes, info: bytes, length: int = 32) -> bytes:
    prk = hmac.new(b"\x00" * 32, ikm, hashlib.sha256).digest()  # RFC 5869: no salt = HashLen zeros
    okm, block = b"", b""
    for i in range(1, -(-length // 32) + 1):
        block = hmac.new(prk, block + info + bytes([i]), hashlib.sha256).digest()
        okm += block
    return okm[:length]


def compute() -> dict:
    out = {}
    # aead.rs: record blob = nonce || ct || tag, AAD = "mossbook/rec/v1" || vault_id || record_id || "entry"
    nonce = bytes(range(24))
    aad = b"mossbook/rec/v1" + VAULT + REC_A + b"entry"
    out["aead"] = nonce + xchacha_encrypt(b"mossbook test vector", aad, nonce, b"\x42" * 32)
    # kdf.rs: Argon2id v0x13 over the NFC-normalized password
    pwd = unicodedata.normalize("NFC", "mossbook").encode()
    out["kdf"] = hash_secret_raw(pwd, b"\x5a" * 16, time_cost=1, memory_cost=64, parallelism=1,
                                 hash_len=32, type=Type.ID, version=19)
    # recovery.rs: KEK_rec = HKDF-SHA256(entropy, info = "mossbook/recovery/v1")
    out["recovery"] = hkdf_sha256(b"\x00" * 32, b"mossbook/recovery/v1")
    # wrap.rs: AAD = "mossbook/wrap/v1" || vault_id
    nonce = bytes(0xF0 ^ i for i in range(24))
    out["wrap"] = nonce + xchacha_encrypt(b"\x02" * 32, b"mossbook/wrap/v1" + VAULT, nonce, b"\x01" * 32)
    return {k: v.hex() for k, v in out.items()}


if __name__ == "__main__":
    ok = True
    for name, value in compute().items():
        match = value == EXPECTED[name]
        ok &= match
        print(f"{'ok  ' if match else 'FAIL'} {name}")
    sys.exit(0 if ok else 1)

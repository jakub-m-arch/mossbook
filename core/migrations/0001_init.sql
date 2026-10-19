-- Mossbook vault schema v1 (SPEC §7).
-- No plaintext dates, names or search fields: everything but ids and type codes is ciphertext.

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value BLOB NOT NULL
) STRICT;

CREATE TABLE records (
  id    BLOB PRIMARY KEY,   -- UUID, 16 bytes
  type  INTEGER NOT NULL,   -- 1 entry, 2 tag, 3 plan, 4 settings
  blob  BLOB NOT NULL       -- nonce || ciphertext || tag
) STRICT;

CREATE TABLE inbox (
  id    BLOB PRIMARY KEY,
  blob  BLOB NOT NULL       -- sealed box (v1, SPEC §6.4)
) STRICT;

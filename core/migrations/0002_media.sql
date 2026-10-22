-- Mossbook vault schema v2: voice notes (E-18, decision D-21).
-- Recordings live apart from `records`, so unlocking does not decrypt them all;
-- each one is decrypted only when it is played. Older builds refuse this schema.

CREATE TABLE media (
  id    BLOB PRIMARY KEY,   -- UUID, 16 bytes
  type  INTEGER NOT NULL,   -- 5 voice
  blob  BLOB NOT NULL       -- nonce || ciphertext || tag, record AAD with type "voice"
) STRICT;

UPDATE meta SET value = x'0200' WHERE key = 'format_version';

-- Mossbook vault schema v3: sync between the user's devices (decision D-22).
-- Deleted entries and tags become tombstones (record type 6) in `records`;
-- `meta` may hold `password_epoch`, `device_id` and `sync_folder`. No table
-- changes, but older builds must not open a vault with tombstones.

UPDATE meta SET value = x'0300' WHERE key = 'format_version';

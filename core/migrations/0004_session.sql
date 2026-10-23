-- Mossbook vault schema v4: preparing for a therapy session (decision D-26).
-- Entries may be of the type `session` and carry `discuss`. No table changes,
-- but an older build would count session entries as unreadable records, so it
-- must refuse the file instead.

UPDATE meta SET value = x'0400' WHERE key = 'format_version';

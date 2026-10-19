//! SQLite storage of the vault file (SPEC §7).
//!
//! This layer only moves opaque bytes: header values in `meta` and encrypted
//! blobs in `records`. It never sees plaintext.

mod migrations;

use core::fmt;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

pub use migrations::SCHEMA_VERSION;

/// Storage error. Carries no SQL text, paths or data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// A vault file already exists at the target path.
    AlreadyExists,
    /// The file was written by a newer version of Mossbook.
    NewerSchema,
    /// Any other SQLite or I/O failure.
    Sqlite,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AlreadyExists => "vault file already exists",
            Self::NewerSchema => "vault was created by a newer version",
            Self::Sqlite => "storage error",
        })
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(_: rusqlite::Error) -> Self {
        // The rusqlite error may contain SQL text; it is dropped on purpose.
        Self::Sqlite
    }
}

/// A raw row of the `records` table.
#[derive(Clone, PartialEq, Eq)]
pub struct RawRecord {
    pub id: [u8; 16],
    pub type_code: u8,
    pub blob: Vec<u8>,
}

impl fmt::Debug for RawRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawRecord")
            .field("type_code", &self.type_code)
            .field("blob_len", &self.blob.len())
            .finish_non_exhaustive()
    }
}

/// Result of reading the `records` table.
#[derive(Debug, Default)]
pub struct RecordScan {
    pub records: Vec<RawRecord>,
    pub malformed: usize,
}

/// Open connection to a vault file.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Creates a new vault file. Fails if the file already exists.
    pub fn create(path: &Path) -> Result<Self, StoreError> {
        if path.exists() {
            return Err(StoreError::AlreadyExists);
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        Self::init(Connection::open_with_flags(path, flags)?)
    }

    /// Opens an existing vault file and applies pending migrations.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        Self::init(Connection::open_with_flags(path, flags)?)
    }

    fn init(mut conn: Connection) -> Result<Self, StoreError> {
        // secure_delete overwrites freed pages with zeros; WAL is checkpointed on close.
        conn.pragma_update(None, "secure_delete", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        migrations::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Reads a header value.
    pub fn meta_get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// Writes several header values atomically.
    pub fn meta_set_all(&mut self, values: &[(&str, &[u8])]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for (key, value) in values {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Inserts or replaces a record.
    pub fn record_put(&mut self, record: &RawRecord) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO records (id, type, blob) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET type = excluded.type, blob = excluded.blob",
            params![record.id.as_slice(), record.type_code, record.blob],
        )?;
        Ok(())
    }

    /// Inserts or replaces many records in one transaction.
    pub fn record_put_all(&mut self, records: &[RawRecord]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO records (id, type, blob) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET type = excluded.type, blob = excluded.blob",
            )?;
            for r in records {
                stmt.execute(params![r.id.as_slice(), r.type_code, r.blob])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Deletes a record. Returns whether it existed.
    pub fn record_delete(&mut self, id: &[u8; 16]) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .execute("DELETE FROM records WHERE id = ?1", [id.as_slice()])?
            > 0)
    }

    /// Returns all well-formed records and the number of malformed rows skipped.
    ///
    /// A malformed row must not lock the user out of every other entry.
    pub fn records_all(&self) -> Result<RecordScan, StoreError> {
        let mut stmt = self.conn.prepare("SELECT id, type, blob FROM records")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        let mut scan = RecordScan::default();
        for row in rows {
            let (id, type_code, blob) = row?;
            match (<[u8; 16]>::try_from(id.as_slice()), u8::try_from(type_code)) {
                (Ok(id), Ok(type_code)) => scan.records.push(RawRecord {
                    id,
                    type_code,
                    blob,
                }),
                _ => scan.malformed += 1,
            }
        }
        Ok(scan)
    }

    /// Rewrites the file to drop freed pages; call after mass deletion (SPEC §7).
    pub fn vacuum(&self) -> Result<(), StoreError> {
        self.conn.execute_batch("VACUUM")?;
        Ok(())
    }

    /// Checkpoints the WAL into the main file and truncates it, then closes the connection.
    pub fn close(self) -> Result<(), StoreError> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        self.conn.close().map_err(|(_, e)| StoreError::from(e))
    }

    #[cfg(test)]
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: u8, type_code: u8, blob: &[u8]) -> RawRecord {
        RawRecord {
            id: [id; 16],
            type_code,
            blob: blob.to_vec(),
        }
    }

    #[test]
    fn create_refuses_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.db");
        Store::create(&path).unwrap().close().unwrap();
        assert_eq!(Store::create(&path).err(), Some(StoreError::AlreadyExists));
    }

    #[test]
    fn schema_and_pragmas() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("vault.db")).unwrap();
        let conn = store.conn();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        let journal: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        let secure: i64 = conn
            .pragma_query_value(None, "secure_delete", |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(journal, "wal");
        assert_eq!(secure, 1);
    }

    #[test]
    fn reopening_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.db");
        let mut store = Store::create(&path).unwrap();
        store.record_put(&record(1, 1, b"x")).unwrap();
        store.close().unwrap();
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.records_all().unwrap().records,
            vec![record(1, 1, b"x")]
        );
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.db");
        let store = Store::create(&path).unwrap();
        store
            .conn()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        store.close().unwrap();
        assert_eq!(Store::open(&path).err(), Some(StoreError::NewerSchema));
    }

    #[test]
    fn open_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Store::open(&dir.path().join("missing.db")).is_err());
        assert!(!dir.path().join("missing.db").exists());
    }

    #[test]
    fn meta_and_records_crud() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("vault.db")).unwrap();
        assert_eq!(store.meta_get("k").unwrap(), None);
        store.meta_set_all(&[("k", b"v1"), ("j", b"w")]).unwrap();
        store.meta_set_all(&[("k", b"v2")]).unwrap();
        assert_eq!(store.meta_get("k").unwrap().as_deref(), Some(&b"v2"[..]));

        store
            .record_put_all(&[record(1, 1, b"a"), record(2, 2, b"b")])
            .unwrap();
        store.record_put(&record(1, 1, b"a2")).unwrap();
        let mut all = store.records_all().unwrap().records;
        all.sort_by_key(|r| r.id);
        assert_eq!(all, vec![record(1, 1, b"a2"), record(2, 2, b"b")]);
        assert!(store.record_delete(&[1; 16]).unwrap());
        assert!(!store.record_delete(&[1; 16]).unwrap());
        store.vacuum().unwrap();
        assert_eq!(store.records_all().unwrap().records.len(), 1);
    }

    #[test]
    fn malformed_rows_are_skipped_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("vault.db")).unwrap();
        store.record_put(&record(1, 1, b"ok")).unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO records (id, type, blob) VALUES (x'01', 1, x'00')",
                [],
            )
            .unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO records (id, type, blob) VALUES (zeroblob(16), 999, x'00')",
                [],
            )
            .unwrap();
        let scan = store.records_all().unwrap();
        assert_eq!(scan.records, vec![record(1, 1, b"ok")]);
        assert_eq!(scan.malformed, 2);
    }

    #[test]
    fn debug_hides_blob() {
        let dbg = format!("{:?}", record(1, 1, b"secret"));
        assert!(!dbg.contains("115"), "{dbg}"); // no raw byte values
        assert!(dbg.contains("blob_len: 6"));
    }
}

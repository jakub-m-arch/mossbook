//! SQL schema migrations, tracked with `PRAGMA user_version`.

use rusqlite::Connection;

use super::StoreError;

/// Migrations in order; migration `i` brings the schema to version `i + 1`.
const MIGRATIONS: &[&str] = &[include_str!("../../migrations/0001_init.sql")];

/// Schema version written by this build.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// Brings the schema up to [`SCHEMA_VERSION`], each step in its own transaction.
///
/// Refuses a database written by a newer build instead of touching it.
pub(super) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(StoreError::NewerSchema);
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

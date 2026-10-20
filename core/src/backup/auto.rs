//! Automatic local backups (decision D-16).
//!
//! When the journal locks, at most once per [`INTERVAL`], a regular `.mossbook`
//! backup is written to a `backups` folder next to the vault, and the newest
//! [`KEEP`] are kept. They protect against a damaged vault file or a mistake
//! (deleting entries, a bad import), not against losing the device: that still
//! needs a backup the user saves elsewhere.
//!
//! The files are ordinary backups (same format, same encryption), so they open
//! with the password or recovery key valid when they were written. File names
//! are fixed slots (`auto-1.mossbook` … `auto-7.mossbook`) so no date appears in
//! plaintext; the files' modification times are visible, as for the vault itself.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{Backup, BackupError};
use crate::vault::VaultError;

/// Folder name, next to the vault file.
pub const DIR_NAME: &str = "backups";
/// How many automatic backups are kept.
pub const KEEP: usize = 7;
/// Minimum time between automatic backups. A little under a day, so a daily
/// habit at a slightly earlier hour still gets its backup.
pub const INTERVAL: Duration = Duration::from_secs(20 * 60 * 60);

const TEMP_NAME: &str = "auto.tmp";

/// An automatic backup on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoBackup {
    pub path: PathBuf,
    pub modified: SystemTime,
}

/// The automatic backup folder for the vault at `vault_path`.
pub fn dir_for(vault_path: &Path) -> PathBuf {
    vault_path.with_file_name(DIR_NAME)
}

fn slot(dir: &Path, n: usize) -> PathBuf {
    dir.join(format!("auto-{n}.mossbook"))
}

/// Automatic backups in `dir`, newest first. A missing folder means none.
pub fn list(dir: &Path) -> Vec<AutoBackup> {
    let mut found: Vec<AutoBackup> = (1..=KEEP)
        .filter_map(|n| {
            let path = slot(dir, n);
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            Some(AutoBackup { path, modified })
        })
        .collect();
    found.sort_by_key(|b| std::cmp::Reverse(b.modified));
    found
}

/// Whether a new automatic backup is due at `now`. A newest backup dated in the
/// future (the clock was moved back) does not count as due, so a wrong clock
/// cannot rotate the older backups away one lock after another.
pub fn is_due(dir: &Path, now: SystemTime) -> bool {
    list(dir).first().is_none_or(|newest| {
        now.duration_since(newest.modified)
            .is_ok_and(|age| age >= INTERVAL)
    })
}

/// Writes `backup` into a free slot, or over the oldest one. The file is
/// written to a temporary name and renamed, so a crash never leaves a
/// half-written backup in a slot.
pub fn write(dir: &Path, backup: &Backup) -> Result<PathBuf, BackupError> {
    let bytes = backup.to_bytes()?;
    write_bytes(dir, &bytes).map_err(|_| BackupError::Vault(VaultError::Io))
}

fn write_bytes(dir: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let target = (1..=KEEP)
        .map(|n| slot(dir, n))
        .find(|p| !p.exists())
        .or_else(|| list(dir).pop().map(|oldest| oldest.path))
        .unwrap_or_else(|| slot(dir, 1));
    let temp = dir.join(TEMP_NAME);
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, &target)?;
    Ok(target)
}

/// Removes all automatic backups (deleting the vault, or after a password
/// change, so old backups do not keep opening with the old password).
pub fn clear(dir: &Path) -> Result<(), BackupError> {
    let io = |_| BackupError::Vault(VaultError::Io);
    for path in (1..=KEEP)
        .map(|n| slot(dir, n))
        .chain([dir.join(TEMP_NAME)])
    {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(e)),
        }
    }
    // Only succeeds if nothing else is in the folder; that is fine either way.
    let _ = fs::remove_dir(dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_modified(path: &Path, t: SystemTime) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn rotates_over_the_oldest_slot() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().join(DIR_NAME);
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        for i in 0..KEEP {
            let path = write_bytes(&dir, &[i as u8]).unwrap();
            set_modified(&path, base + Duration::from_secs(i as u64 * 100));
        }
        assert_eq!(list(&dir).len(), KEEP);
        let oldest = list(&dir).pop().unwrap().path;
        let written = write_bytes(&dir, b"new").unwrap();
        assert_eq!(written, oldest);
        assert_eq!(fs::read(&written).unwrap(), b"new");
        assert_eq!(list(&dir).len(), KEEP);
        assert!(!dir.join(TEMP_NAME).exists());
    }

    #[test]
    fn due_after_the_interval_only() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().join(DIR_NAME);
        let now = SystemTime::now();
        assert!(is_due(&dir, now), "no backup yet");
        let path = write_bytes(&dir, b"x").unwrap();
        set_modified(&path, now - INTERVAL + Duration::from_secs(60));
        assert!(!is_due(&dir, now));
        set_modified(&path, now - INTERVAL);
        assert!(is_due(&dir, now));
        set_modified(&path, now + Duration::from_secs(3600));
        assert!(!is_due(&dir, now), "a future date is not due");
    }

    #[test]
    fn clear_removes_only_automatic_backups() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join(DIR_NAME);
        write_bytes(&backups, b"x").unwrap();
        fs::write(backups.join("mine.mossbook"), b"user file").unwrap();
        clear(&backups).unwrap();
        assert!(list(&backups).is_empty());
        assert!(backups.join("mine.mossbook").exists());
        // A missing folder is fine.
        clear(&dir.path().join("nothing")).unwrap();
    }
}

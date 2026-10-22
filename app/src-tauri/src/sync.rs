//! The sync folder (decision D-22): where this device writes its sync file and
//! reads the others'. Syncthing (or anything the user chooses) carries the
//! folder between devices; Mossbook only reads and writes files in it.

use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

/// Sync files end with this; anything else in the folder is ignored.
pub const SUFFIX: &str = ".mossbook-sync";

/// A file in the sync folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub name: String,
    pub modified: SystemTime,
    pub len: u64,
}

/// A folder this device can list, read and write sync files in.
pub trait Folder: Send {
    /// The sync files in the folder.
    fn list(&self) -> io::Result<Vec<FileInfo>>;
    fn read(&self, name: &str) -> io::Result<Vec<u8>>;
    /// Replaces `name` so that a reader never sees half a file.
    fn write(&self, name: &str, bytes: &[u8]) -> io::Result<()>;
    /// Removes `name` if it exists.
    fn remove(&self, name: &str) -> io::Result<()>;
}

/// The file name of a device's sync file.
pub fn file_name(device_id: &str) -> String {
    format!("{device_id}{SUFFIX}")
}

/// The folder the user picked in the system dialog, and how to show it.
pub fn open_folder(
    _app: &tauri::AppHandle,
    picked: tauri_plugin_dialog::FilePath,
) -> crate::error::AppResult<(Box<dyn Folder>, String)> {
    let path = picked
        .into_path()
        .map_err(|_| crate::error::AppError::new("folder_unavailable", "not a folder path"))?;
    let location = path.to_string_lossy().into_owned();
    Ok((Box::new(DirFolder(path)), location))
}

/// Reconnects a folder recorded on this device, at start-up.
pub fn reopen_folder(_app: &tauri::AppHandle, location: &str) -> Option<Box<dyn Folder>> {
    Some(Box::new(DirFolder(PathBuf::from(location))))
}

/// A folder on a desktop file system.
pub struct DirFolder(pub PathBuf);

impl Folder for DirFolder {
    fn list(&self) -> io::Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&self.0)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            // Syncthing's own temporary and conflict copies are not sync files.
            if !name.ends_with(SUFFIX) || name.starts_with('.') || name.contains(".sync-conflict") {
                continue;
            }
            let meta = entry.metadata()?;
            if meta.is_file() {
                files.push(FileInfo {
                    name,
                    modified: meta.modified()?,
                    len: meta.len(),
                });
            }
        }
        Ok(files)
    }

    fn read(&self, name: &str) -> io::Result<Vec<u8>> {
        std::fs::read(self.0.join(checked(name)?))
    }

    fn write(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write;
        let name = checked(name)?;
        // A dot-file is ignored by readers (above) and by Syncthing's default
        // patterns until it is renamed into place.
        let tmp = self.0.join(format!(".{name}.tmp"));
        {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, self.0.join(name))
    }

    fn remove(&self, name: &str) -> io::Result<()> {
        match std::fs::remove_file(self.0.join(checked(name)?)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// A plain file name: no separators or parent references.
fn checked(name: &str) -> io::Result<&str> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad file name"));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_sync_files_and_writes_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let folder = DirFolder(dir.path().to_path_buf());
        folder.write(&file_name("phone"), b"one").unwrap();
        folder.write(&file_name("phone"), b"two").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        std::fs::write(
            dir.path().join("pc.sync-conflict-20261022-1.mossbook-sync"),
            b"x",
        )
        .unwrap();
        let names: Vec<_> = folder.list().unwrap().into_iter().map(|f| f.name).collect();
        assert_eq!(names, vec!["phone.mossbook-sync".to_owned()]);
        assert_eq!(folder.read("phone.mossbook-sync").unwrap(), b"two");
        assert!(folder.read("../vault.db").is_err());
        assert!(folder.write("a/b.mossbook-sync", b"x").is_err());
    }
}

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
    /// How the folder is shown in the settings.
    fn label(&self) -> String;
}

/// The file name of a device's sync file.
pub fn file_name(device_id: &str) -> String {
    format!("{device_id}{SUFFIX}")
}

pub use platform::{init, pick_folder, reopen_folder};

#[cfg(not(target_os = "android"))]
mod platform {
    use std::path::PathBuf;

    use tauri::plugin::{Builder, TauriPlugin};
    use tauri::{AppHandle, Runtime};
    use tauri_plugin_dialog::DialogExt;

    use super::{DirFolder, Folder};
    use crate::error::{AppError, AppResult};

    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        Builder::new("sync-folder").build()
    }

    /// Asks for the sync folder in the system dialog: the folder and where it
    /// is, to keep on this device. `None` if cancelled.
    pub fn pick_folder(app: &AppHandle) -> AppResult<Option<(Box<dyn Folder>, String)>> {
        let Some(picked) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        let path = picked
            .into_path()
            .map_err(|_| AppError::new("folder_unavailable", "not a folder path"))?;
        let location = path.to_string_lossy().into_owned();
        Ok(Some((Box::new(DirFolder(path)), location)))
    }

    /// Reconnects a folder recorded on this device, at start-up.
    pub fn reopen_folder(_app: &AppHandle, location: &str) -> Option<Box<dyn Folder>> {
        Some(Box::new(DirFolder(PathBuf::from(location))))
    }
}

/// Android: a folder from the system's folder picker, through the Kotlin
/// plugin `SyncFolderPlugin` (Storage Access Framework).
#[cfg(target_os = "android")]
mod platform {
    use std::io;
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    use serde::{Deserialize, Serialize};
    use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
    use tauri::{AppHandle, Manager, Wry};

    use super::{FileInfo, Folder};
    use crate::error::{AppError, AppResult};

    struct Handle(PluginHandle<Wry>);

    pub fn init() -> TauriPlugin<Wry> {
        Builder::new("sync-folder")
            .setup(|app, api| {
                let handle = api
                    .register_android_plugin("io.github.jakubmarch.mossbook", "SyncFolderPlugin")?;
                app.manage(Handle(handle));
                Ok(())
            })
            .build()
    }

    fn handle(app: &AppHandle<Wry>) -> io::Result<PluginHandle<Wry>> {
        app.try_state::<Handle>()
            .map(|h| h.inner().0.clone())
            .ok_or_else(|| io::Error::other("no plugin"))
    }

    fn call<T: serde::de::DeserializeOwned>(
        app: &AppHandle<Wry>,
        command: &str,
        args: impl Serialize,
    ) -> io::Result<T> {
        handle(app)?
            .run_mobile_plugin(command, args)
            .map_err(|_| io::Error::other("sync folder"))
    }

    #[derive(Deserialize)]
    struct Picked {
        uri: String,
        name: String,
    }

    #[derive(Deserialize)]
    struct Named {
        name: String,
    }

    #[derive(Serialize)]
    struct FolderArgs<'a> {
        uri: &'a str,
    }

    #[derive(Serialize)]
    struct FileArgs<'a> {
        uri: &'a str,
        name: &'a str,
        path: Option<String>,
    }

    #[derive(Deserialize)]
    struct Listed {
        files: Vec<Listing>,
    }

    #[derive(Deserialize)]
    struct Listing {
        name: String,
        /// Milliseconds since the Unix epoch.
        modified: u64,
        len: u64,
    }

    pub struct SafFolder {
        app: AppHandle<Wry>,
        uri: String,
        name: String,
        cache: PathBuf,
    }

    impl SafFolder {
        fn new(app: &AppHandle<Wry>, uri: String, name: String) -> Option<Self> {
            let cache = app.path().app_cache_dir().ok()?.join("sync");
            std::fs::create_dir_all(&cache).ok()?;
            Some(Self {
                app: app.clone(),
                uri,
                name,
                cache,
            })
        }

        fn scratch(&self, name: &str) -> PathBuf {
            self.cache.join(name)
        }
    }

    impl Folder for SafFolder {
        fn list(&self) -> io::Result<Vec<FileInfo>> {
            let listed: Listed = call(&self.app, "list", FolderArgs { uri: &self.uri })?;
            Ok(listed
                .files
                .into_iter()
                .map(|f| FileInfo {
                    name: f.name,
                    modified: UNIX_EPOCH + Duration::from_millis(f.modified),
                    len: f.len,
                })
                .collect())
        }

        fn read(&self, name: &str) -> io::Result<Vec<u8>> {
            let path = self.scratch(name);
            let (): () = call(
                &self.app,
                "read",
                FileArgs {
                    uri: &self.uri,
                    name,
                    path: Some(path.to_string_lossy().into_owned()),
                },
            )?;
            let bytes = std::fs::read(&path);
            let _ = std::fs::remove_file(&path);
            bytes
        }

        fn write(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
            let path = self.scratch(name);
            std::fs::write(&path, bytes)?;
            let result: io::Result<()> = call(
                &self.app,
                "write",
                FileArgs {
                    uri: &self.uri,
                    name,
                    path: Some(path.to_string_lossy().into_owned()),
                },
            );
            let _ = std::fs::remove_file(&path);
            result
        }

        fn remove(&self, name: &str) -> io::Result<()> {
            let (): () = call(
                &self.app,
                "remove",
                FileArgs {
                    uri: &self.uri,
                    name,
                    path: None,
                },
            )?;
            Ok(())
        }

        fn label(&self) -> String {
            self.name.clone()
        }
    }

    pub fn pick_folder(app: &AppHandle<Wry>) -> AppResult<Option<(Box<dyn Folder>, String)>> {
        // Cancelling the picker and an unusable folder both leave sync off.
        let Ok(picked) = call::<Picked>(app, "pick", ()) else {
            return Ok(None);
        };
        let folder = SafFolder::new(app, picked.uri.clone(), picked.name)
            .ok_or_else(|| AppError::new("folder_unavailable", "no cache"))?;
        Ok(Some((Box::new(folder), picked.uri)))
    }

    pub fn reopen_folder(app: &AppHandle<Wry>, location: &str) -> Option<Box<dyn Folder>> {
        let name = call::<Named>(app, "name", FolderArgs { uri: location })
            .map(|n| n.name)
            .unwrap_or_default();
        Some(Box::new(SafFolder::new(app, location.to_owned(), name)?))
    }
}

/// A folder on a desktop file system (on Android only in tests).
#[cfg_attr(target_os = "android", allow(dead_code))]
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

    fn label(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

/// A plain file name: no separators or parent references.
#[cfg_attr(target_os = "android", allow(dead_code))]
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

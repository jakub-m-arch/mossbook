//! Voice note transcription on this device (E-19, D-23): where the model lives,
//! one job at a time (an import or a transcription) with a way to cancel it,
//! and the memory figure behind the warning in the settings.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use mossbook_core::transcribe::{PARAKEET_V3, model};
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// Below this much memory the settings warn that transcribing may fail (the
/// model needs about 1.5 GB while it runs). Phones sold with 6 GB report a
/// little less than that to apps, so the line sits at 5 GiB.
pub const LOW_MEMORY: u64 = 5 * 1024 * 1024 * 1024;

pub struct Transcription {
    root: PathBuf,
    job: Mutex<Option<Arc<AtomicBool>>>,
}

/// What the settings show about transcription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TranscriptionStatus {
    /// The model is imported, so transcription is on.
    pub installed: bool,
    /// An import or a transcription is running.
    pub busy: bool,
    /// Size of the file the user downloads.
    pub download_bytes: u64,
    /// Space the model takes once imported.
    pub disk_bytes: u64,
    /// Total memory of the device, where it can be read (Android, Linux).
    pub total_memory: Option<u64>,
    pub low_memory: bool,
}

/// The running job; ends when dropped.
pub struct Job<'a> {
    owner: &'a Transcription,
    cancel: Arc<AtomicBool>,
}

impl Job<'_> {
    pub fn cancelled(&self) -> &AtomicBool {
        &self.cancel
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Drop for Job<'_> {
    fn drop(&mut self) {
        *self.owner.slot() = None;
    }
}

impl Transcription {
    /// `root` is the folder models are imported into.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            job: Mutex::new(None),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn slot(&self) -> std::sync::MutexGuard<'_, Option<Arc<AtomicBool>>> {
        self.job.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn status(&self) -> TranscriptionStatus {
        let total_memory = total_memory();
        TranscriptionStatus {
            installed: model::is_installed(&self.root, &PARAKEET_V3),
            busy: self.slot().is_some(),
            download_bytes: PARAKEET_V3.archive_bytes,
            disk_bytes: PARAKEET_V3.files.iter().map(|(_, bytes)| bytes).sum(),
            total_memory,
            low_memory: total_memory.is_some_and(|m| m < LOW_MEMORY),
        }
    }

    /// Starts a job, unless one is already running.
    pub fn start(&self) -> AppResult<Job<'_>> {
        let mut slot = self.slot();
        if slot.is_some() {
            return Err(AppError::new("busy", "a transcription job is running"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(Arc::clone(&cancel));
        Ok(Job {
            owner: self,
            cancel,
        })
    }

    /// Asks the running job, if any, to stop.
    pub fn cancel(&self) {
        if let Some(flag) = self.slot().as_ref() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Removes the model, which turns transcription off.
    pub fn remove(&self) -> AppResult<()> {
        let _job = self.start()?;
        model::remove(&self.root, &PARAKEET_V3)
            .map_err(|_| AppError::new("io", "cannot remove the model"))
    }
}

/// Total memory from `/proc/meminfo` (Android and Linux). Elsewhere `None`,
/// and no warning is shown: desktops have far more than the model needs.
fn total_memory() -> Option<u64> {
    if cfg!(any(target_os = "android", target_os = "linux")) {
        parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
    } else {
        None
    }
}

fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let mut parts = line.split_whitespace().skip(1);
    let kib: u64 = parts.next()?.parse().ok()?;
    (parts.next() == Some("kB")).then_some(kib * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_job_at_a_time() {
        let t = Transcription::new(PathBuf::from("/nonexistent"));
        let job = t.start().unwrap();
        assert!(t.status().busy);
        assert_eq!(t.start().err().map(|e| e.code), Some("busy"));
        assert_eq!(t.remove().err().map(|e| e.code), Some("busy"));
        assert!(!job.cancelled().load(Ordering::Relaxed));
        t.cancel();
        assert!(job.cancelled().load(Ordering::Relaxed));
        drop(job);
        assert!(!t.status().busy);
        let next = t.start().unwrap();
        assert!(
            !next.cancelled().load(Ordering::Relaxed),
            "a new job starts afresh"
        );
    }

    #[test]
    fn status_without_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let t = Transcription::new(dir.path().to_path_buf());
        let s = t.status();
        assert!(!s.installed && !s.busy);
        assert_eq!(s.download_bytes, 487_170_055);
        assert_eq!(s.disk_bytes, 670_478_772);
        t.remove().unwrap();
        let v = serde_json::to_value(&s).unwrap();
        for key in [
            "installed",
            "busy",
            "download_bytes",
            "disk_bytes",
            "total_memory",
            "low_memory",
        ] {
            assert!(v.get(key).is_some(), "{key}");
        }
    }

    #[test]
    fn memory_is_read_from_meminfo() {
        let text = "MemTotal:        3809580 kB\nMemFree:          123456 kB\n";
        assert_eq!(parse_meminfo(text), Some(3_809_580 * 1024));
        assert!(parse_meminfo(text).unwrap() < LOW_MEMORY, "a 4 GB phone");
        assert!(
            parse_meminfo("MemTotal: 5621000 kB").unwrap() > LOW_MEMORY,
            "a 6 GB phone"
        );
        assert_eq!(parse_meminfo("MemFree: 1 kB\n"), None);
        assert_eq!(parse_meminfo("MemTotal: lots\n"), None);
        assert_eq!(parse_meminfo("MemTotal: 12 MB\n"), None);
    }
}

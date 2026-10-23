//! Vault session: the state machine behind the commands and the auto-lock
//! policy (SPEC §8.3, V-3, decisions D-6 and D-11). Plain Rust, no Tauri, so
//! it can be unit-tested.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use mossbook_core::backup::{Backup, BackupError, auto};
use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::domain::{AutoLock, EntryKindTag, Timestamp};
use mossbook_core::journal::{ImportMode, ImportReport, Journal, SyncReport};
use mossbook_core::vault::{
    self, DestroyError, DeviceEnrollment, DeviceKeyFile, Local, PendingVault, UnlockThrottle,
    VaultError,
};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset};

use crate::error::{AppError, AppResult};
use crate::sync::{Folder, file_name};

/// Inactivity timeout when the policy is "immediately": locking on background
/// covers phones, and on desktop (where background is fuzzy) this timer does (D-11).
pub const IMMEDIATE_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How long a background lock waits for a voice note being recorded to be
/// stored and the form kept (D-25).
pub const RECORDING_GRACE: Duration = Duration::from_secs(3);
/// Longest a recording can keep the idle timer away: the 5-minute limit (D-21)
/// plus slack. A flag the UI never cleared stops counting after this.
const RECORDING_SPAN: Duration = Duration::from_secs(5 * 60 + 30);

enum VaultState {
    NoVault,
    /// Created in memory, waiting for the recovery words (D-3).
    Creating(Box<PendingVault>),
    Locked,
    Unlocked(Box<Journal>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusKind {
    NoVault,
    Creating,
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub state: StatusKind,
    /// Records that could not be read on unlock (0 when not unlocked).
    pub unreadable_records: usize,
    /// Remaining wait before the next unlock attempt, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

/// Shown once while creating a vault.
#[derive(Serialize)]
pub struct RecoverySetup {
    pub words: Vec<String>,
    /// 0-based positions the user has to retype.
    pub positions: [usize; 3],
}

#[derive(Deserialize)]
pub struct RecoveryAnswer {
    pub position: usize,
    pub word: String,
}

/// Why a lock happened, sent to the UI with the `vault-locked` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockReason {
    Manual,
    Background,
    Idle,
}

pub struct Session {
    vault_path: PathBuf,
    state: VaultState,
    throttle: UnlockThrottle,
    last_activity: Instant,
    /// System dialogs (file pickers) currently open; they suspend the
    /// background lock but not the idle timer (D-6).
    dialogs_open: u32,
    /// A voice note is being recorded, since then (D-25). Counts as activity.
    recording_since: Option<Instant>,
    /// The app went to the background while recording: lock once the UI says
    /// the recording is stored, at the latest at this instant (D-25).
    deferred_lock: Option<Instant>,
    utc_offset: UtcOffset,
    /// Cheap KDF params for tests; `None` in the app (calibrated params).
    kdf_override: Option<KdfParams>,
    /// A backup file picked by the user, waiting for its password (X-2).
    pending_backup: Option<Backup>,
    /// The last automatic backup that was due could not be written (D-16).
    auto_backup_failed: bool,
    /// This device's sync folder, when sync is on (D-22).
    sync: Option<SyncLink>,
}

/// This device's link to its sync folder (D-22).
struct SyncLink {
    folder: Box<dyn Folder>,
    /// Where the folder is, as shown in the settings.
    location: String,
    /// Names this device's sync file.
    device: String,
    /// What the journal looked like when this device last wrote its file.
    written: Option<[u8; 32]>,
    /// Other devices' files as last merged: (modified, length).
    seen: HashMap<String, (SystemTime, u64)>,
    last_sync: Option<OffsetDateTime>,
    /// Why the last sync did not complete, for the settings.
    problem: Option<&'static str>,
    /// The folder holds a file of another journal: the user can join it.
    other_journal: bool,
    /// Entries, tags, plan or settings the last sync changed here.
    received: usize,
}

/// What the settings show about sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncStatus {
    /// The folder, when sync is on.
    pub location: Option<String>,
    /// The devices with a file in the folder, this one included.
    pub devices: Vec<SyncDevice>,
    pub other_journal: bool,
    pub last_sync: Option<Timestamp>,
    pub problem: Option<&'static str>,
    /// What the last sync changed here (the UI reloads when it is not 0).
    pub received: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncDevice {
    pub this_device: bool,
    /// When its file last changed, in the user's offset.
    pub changed_at: Timestamp,
    /// The file belongs to another journal.
    pub other_journal: bool,
}

/// What the settings show about backups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupInfo {
    pub auto_enabled: bool,
    /// Newest automatic backup on this device, in the user's offset.
    pub auto_latest: Option<Timestamp>,
    pub auto_count: usize,
    pub auto_failed: bool,
    /// Whether the system file manager can show the backups folder. Not on
    /// mobile, where the backups live in the app's private storage.
    pub can_reveal: bool,
    /// When the user last saved a backup themselves.
    pub last_export: Option<Timestamp>,
}

/// What the UI learns about a picked backup before it is opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupSummary {
    pub records: usize,
    /// The backup comes from the vault that is currently unlocked.
    pub same_vault: bool,
}

/// Which secret opens a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupSecret {
    Password,
    RecoveryPhrase,
}

/// Result of bringing a backup in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportOutcome {
    /// `true` when there was no vault and the backup became the vault.
    pub restored: bool,
    pub report: ImportReport,
}

impl Session {
    pub fn new(vault_path: PathBuf) -> Self {
        let state = if vault::exists(&vault_path) {
            VaultState::Locked
        } else {
            VaultState::NoVault
        };
        Self {
            vault_path,
            state,
            throttle: UnlockThrottle::new(),
            last_activity: Instant::now(),
            dialogs_open: 0,
            recording_since: None,
            deferred_lock: None,
            utc_offset: UtcOffset::UTC,
            kdf_override: None,
            pending_backup: None,
            auto_backup_failed: false,
            sync: None,
        }
    }

    #[cfg(test)]
    fn with_fast_kdf(vault_path: PathBuf) -> Self {
        Self {
            kdf_override: Some(KdfParams::new(64, 1, 1).unwrap()),
            ..Self::new(vault_path)
        }
    }

    pub fn status(&self, now: Instant) -> Status {
        let (state, unreadable_records) = match &self.state {
            VaultState::NoVault => (StatusKind::NoVault, 0),
            VaultState::Creating(_) => (StatusKind::Creating, 0),
            VaultState::Locked => (StatusKind::Locked, 0),
            VaultState::Unlocked(j) => (StatusKind::Unlocked, j.vault().unreadable_records()),
        };
        let retry_after_ms = self
            .throttle
            .check(now)
            .err()
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        Status {
            state,
            unreadable_records,
            retry_after_ms,
        }
    }

    // ---- clock ----

    /// The UI reports its UTC offset (the system clock's local offset cannot be
    /// read reliably from a multi-threaded process on every platform).
    pub fn set_utc_offset(&mut self, minutes: i32) -> AppResult<()> {
        // Real time zones range from UTC-12:00 to UTC+14:00.
        if !(-12 * 60..=14 * 60).contains(&minutes) {
            return Err(AppError::new("invalid_field", "offset"));
        }
        self.utc_offset = UtcOffset::from_whole_seconds(minutes * 60)
            .map_err(|_| AppError::new("invalid_field", "offset"))?;
        Ok(())
    }

    /// "Now" in the user's local offset.
    pub fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc().to_offset(self.utc_offset)
    }

    // ---- creation ----

    /// Prepares a new vault in memory and returns the recovery phrase.
    /// Calling it again while creating starts over with a new phrase.
    pub fn create(&mut self, password: &str) -> AppResult<RecoverySetup> {
        if !matches!(self.state, VaultState::NoVault | VaultState::Creating(_)) {
            return Err(VaultError::AlreadyExists.into());
        }
        let pending = match self.kdf_override {
            Some(params) => PendingVault::with_kdf_params(password, params)?,
            None => PendingVault::new(password)?,
        };
        let setup = RecoverySetup {
            words: pending
                .recovery_phrase()
                .split(' ')
                .map(str::to_owned)
                .collect(),
            positions: pending.confirmation_positions(),
        };
        self.state = VaultState::Creating(Box::new(pending));
        Ok(setup)
    }

    pub fn cancel_create(&mut self) {
        if matches!(self.state, VaultState::Creating(_)) {
            self.state = VaultState::NoVault;
        }
    }

    /// Checks the retyped words and, if they match, writes the vault and unlocks it.
    pub fn confirm_recovery(&mut self, answers: &[RecoveryAnswer], now: Instant) -> AppResult<()> {
        let VaultState::Creating(pending) = &mut self.state else {
            return Err(AppError::invalid_state("no vault is being created"));
        };
        let pairs: Vec<(usize, &str)> = answers
            .iter()
            .map(|a| (a.position, a.word.as_str()))
            .collect();
        if !pending.confirm(&pairs) {
            return Err(AppError::new("recovery_mismatch", "the words do not match"));
        }
        let VaultState::Creating(pending) = std::mem::replace(&mut self.state, VaultState::NoVault)
        else {
            unreachable!("checked above");
        };
        let vault = pending.write(&self.vault_path)?;
        self.state = VaultState::Unlocked(Box::new(Journal::open(vault)));
        self.last_activity = now;
        Ok(())
    }

    // ---- unlock / lock ----

    pub fn unlock(&mut self, password: &str, now: Instant) -> AppResult<()> {
        self.before_unlock(now)?;
        let result = vault::unlock(&self.vault_path, password);
        self.after_unlock(result, now)
    }

    pub fn unlock_with_recovery(
        &mut self,
        phrase: &str,
        new_password: &str,
        now: Instant,
    ) -> AppResult<()> {
        self.before_unlock(now)?;
        let result = vault::unlock_with_recovery(&self.vault_path, phrase, new_password);
        self.after_unlock(result, now)?;
        self.refresh_auto_backups();
        Ok(())
    }

    fn before_unlock(&self, now: Instant) -> AppResult<()> {
        match self.state {
            VaultState::Locked => self.throttle.check(now).map_err(AppError::throttled),
            VaultState::Unlocked(_) => Err(AppError::invalid_state("already unlocked")),
            VaultState::NoVault | VaultState::Creating(_) => Err(VaultError::NotFound.into()),
        }
    }

    fn after_unlock(
        &mut self,
        result: Result<vault::UnlockedVault, VaultError>,
        now: Instant,
    ) -> AppResult<()> {
        match result {
            Ok(v) => {
                self.throttle.record_success();
                self.state = VaultState::Unlocked(Box::new(Journal::open(v)));
                self.last_activity = now;
                // What the other devices wrote while this one was locked.
                self.sync_receive();
                Ok(())
            }
            Err(
                e @ (VaultError::WrongPassword
                | VaultError::WrongRecoveryKey
                | VaultError::InvalidRecoveryPhrase),
            ) => {
                let wait = self.throttle.record_failure(now);
                Err(AppError::from(e).with_retry(wait))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Locks if unlocked. Returns whether a lock happened. A vault being created
    /// is kept: it holds no user data yet, and the user may switch apps to store
    /// the recovery phrase in a password manager.
    pub fn lock(&mut self) -> AppResult<bool> {
        self.recording_since = None;
        self.deferred_lock = None;
        if !matches!(self.state, VaultState::Unlocked(_)) {
            return Ok(false);
        }
        let VaultState::Unlocked(journal) = std::mem::replace(&mut self.state, VaultState::Locked)
        else {
            unreachable!("checked above");
        };
        if journal.settings().auto_backup && auto::is_due(&self.auto_dir(), SystemTime::now()) {
            self.auto_backup_failed = !write_auto_backup(&self.auto_dir(), &journal);
        }
        // Best effort, like the automatic backup: a lock never waits on sync.
        if let Some(link) = &mut self.sync {
            link.problem = send(link, &journal).err();
        }
        // Even if flushing fails, the journal (DEK and content) is dropped and zeroed.
        journal.lock()?;
        Ok(true)
    }

    // ---- automatic backups (D-16) ----

    fn auto_dir(&self) -> PathBuf {
        auto::dir_for(&self.vault_path)
    }

    /// After the password changes, older automatic backups would keep opening
    /// with the old password: replace them with one under the new password.
    fn refresh_auto_backups(&mut self) {
        let dir = self.auto_dir();
        let Ok(journal) = self.journal() else {
            return;
        };
        let enabled = journal.settings().auto_backup;
        let ok = auto::clear(&dir).is_ok() && (!enabled || write_auto_backup(&dir, journal));
        self.auto_backup_failed = !ok;
    }

    /// Where automatic backups are kept: the folder a backup picker opens in.
    pub fn auto_backup_dir(&self) -> Option<PathBuf> {
        let dir = self.auto_dir();
        dir.is_dir().then_some(dir)
    }

    /// The newest automatic backup, to show it in the file manager.
    pub fn latest_auto_backup(&self) -> Option<PathBuf> {
        auto::list(&self.auto_dir())
            .into_iter()
            .next()
            .map(|b| b.path)
    }

    pub fn backup_info(&self) -> AppResult<BackupInfo> {
        let settings = self.journal()?.settings();
        let backups = auto::list(&self.auto_dir());
        Ok(BackupInfo {
            auto_enabled: settings.auto_backup,
            auto_latest: backups.first().map(|b| {
                Timestamp(time::OffsetDateTime::from(b.modified).to_offset(self.utc_offset))
            }),
            auto_count: backups.len(),
            auto_failed: self.auto_backup_failed,
            can_reveal: cfg!(desktop),
            last_export: settings.last_export,
        })
    }

    /// Quick entry from the lock screen (E-15): sealed into the inbox without
    /// the password; it appears in the journal after the next unlock.
    pub fn quick_entry_locked(&self, kind: EntryKindTag, value: Option<u8>) -> AppResult<()> {
        if !matches!(self.state, VaultState::Locked) {
            return Err(AppError::invalid_state("the vault is not locked"));
        }
        Ok(Journal::quick_entry_locked(
            &self.vault_path,
            kind,
            value,
            self.now(),
        )?)
    }

    // ---- password management ----

    pub fn change_password(&mut self, old: &str, new: &str) -> AppResult<()> {
        self.journal_mut()?.vault_mut().change_password(old, new)?;
        self.refresh_auto_backups();
        Ok(())
    }

    // ---- fingerprint unlock (V-7, D-18) ----

    /// Whether a device key file is set up for this vault.
    pub fn device_unlock_enabled(&self) -> bool {
        DeviceKeyFile::path_for(&self.vault_path).exists()
    }

    /// Starts turning fingerprint unlock on: re-checks the password (it is
    /// asked for even though the journal is open), then makes a fresh device key.
    pub fn device_enrollment(&self, password: &str) -> AppResult<DeviceEnrollment> {
        let vault = self.journal()?.vault();
        vault.verify_password(password)?;
        Ok(vault.device_enrollment()?)
    }

    /// Stores the device key file once the platform has encrypted the key.
    pub fn save_device_key(&self, file: &DeviceKeyFile) -> AppResult<()> {
        Ok(file.write(&self.vault_path)?)
    }

    /// Turns fingerprint unlock off on this side (the file).
    pub fn forget_device_key(&self) -> AppResult<()> {
        Ok(DeviceKeyFile::remove(&self.vault_path)?)
    }

    /// The device key file, for asking the platform to open its key. A damaged
    /// file is removed: the password still opens the journal.
    pub fn device_key_file(&self) -> AppResult<DeviceKeyFile> {
        if !matches!(self.state, VaultState::Locked) {
            return Err(AppError::invalid_state("not locked"));
        }
        match DeviceKeyFile::read(&self.vault_path) {
            Ok(Some(file)) => Ok(file),
            Ok(None) => Err(AppError::new(
                "device_unlock_off",
                "fingerprint unlock is off",
            )),
            Err(_) => {
                self.forget_device_key()?;
                Err(AppError::new(
                    "device_unlock_off",
                    "fingerprint unlock is off",
                ))
            }
        }
    }

    /// Opens the journal with the device key released by the platform. `key` is
    /// zeroed. A key that does not open this vault turns fingerprint unlock off.
    /// Not throttled: the platform limits biometric attempts itself.
    pub fn unlock_with_device_key(
        &mut self,
        file: &DeviceKeyFile,
        key: &mut [u8],
        now: Instant,
    ) -> AppResult<()> {
        if !matches!(self.state, VaultState::Locked) {
            zeroize::Zeroize::zeroize(key);
            return self.before_unlock(now);
        }
        match vault::unlock_with_device_key(&self.vault_path, file, key) {
            Ok(v) => self.after_unlock(Ok(v), now),
            Err(VaultError::DeviceKeyRejected) => {
                self.forget_device_key()?;
                Err(VaultError::DeviceKeyRejected.into())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Checks a written-down recovery phrase against the unlocked vault.
    pub fn verify_recovery(&self, phrase: &str) -> AppResult<()> {
        Ok(self.journal()?.vault().verify_recovery(phrase)?)
    }

    /// Deletes the vault after re-checking the password (V-9). On a wrong
    /// password nothing changes and the vault stays unlocked.
    pub fn destroy(&mut self, password: &str) -> AppResult<()> {
        // Check first, while the journal is still intact: handing it over to
        // `destroy` gives up its decoded content.
        self.journal()?.vault().verify_password(password)?;
        let VaultState::Unlocked(journal) = std::mem::replace(&mut self.state, VaultState::NoVault)
        else {
            unreachable!("journal() succeeded above");
        };
        match journal.into_vault().destroy(password) {
            Ok(()) => {
                // The journal is gone; its automatic backups and this device's
                // sync file go with it. The other devices keep their copies.
                auto::clear(&self.auto_dir())?;
                self.auto_backup_failed = false;
                if let Some(link) = self.sync.take() {
                    let _ = link.folder.remove(&file_name(&link.device));
                }
                Ok(())
            }
            Err(DestroyError::Rejected { vault, error }) => {
                // Not reachable after the check above; stay safe and lock.
                vault.lock()?;
                self.state = VaultState::Locked;
                Err(error.into())
            }
            Err(DestroyError::Failed(error)) => {
                self.state = if vault::exists(&self.vault_path) {
                    VaultState::Locked
                } else {
                    VaultState::NoVault
                };
                Err(error.into())
            }
        }
    }

    // ---- sync between the user's devices (D-22) ----

    /// Reconnects the sync folder recorded in the vault, at start-up.
    pub fn attach_sync(&mut self, folder: Box<dyn Folder>, location: String, device: String) {
        self.sync = Some(SyncLink::new(folder, location, device));
    }

    /// Turns sync on with `folder`: this device gets an id if it has none, and
    /// the folder is remembered on this device only. Reads what is there and
    /// writes this device's file.
    pub fn sync_connect(
        &mut self,
        folder: Box<dyn Folder>,
        location: String,
    ) -> AppResult<SyncStatus> {
        let journal = self.journal_mut()?;
        let device = match journal.vault().local(Local::DeviceId)? {
            Some(id) => id,
            None => {
                let id = new_device_id()?;
                journal.vault_mut().set_local(Local::DeviceId, Some(&id))?;
                id
            }
        };
        journal
            .vault_mut()
            .set_local(Local::SyncFolder, Some(&location))?;
        self.sync = Some(SyncLink::new(folder, location, device));
        self.sync_now()
    }

    /// Turns sync off on this device. The folder and the other devices' files
    /// are left as they are.
    pub fn sync_disconnect(&mut self) -> AppResult<()> {
        self.journal_mut()?
            .vault_mut()
            .set_local(Local::SyncFolder, None)?;
        self.sync = None;
        Ok(())
    }

    /// Merges the other devices' files and writes this device's.
    pub fn sync_now(&mut self) -> AppResult<SyncStatus> {
        self.journal()?;
        self.sync_receive();
        if let (Some(link), VaultState::Unlocked(journal)) = (&mut self.sync, &self.state) {
            let sent = send(link, journal);
            if link.problem.is_none() {
                link.problem = sent.err();
            }
        }
        Ok(self.sync_status())
    }

    /// Merges the other devices' files; problems are kept for the settings.
    fn sync_receive(&mut self) -> Option<SyncReport> {
        let now = self.now();
        let (Some(link), VaultState::Unlocked(journal)) = (&mut self.sync, &mut self.state) else {
            return None;
        };
        match receive(link, journal, now) {
            Ok(report) => {
                link.problem = None;
                link.last_sync = Some(now);
                link.received = report.changed + report.deleted;
                Some(report)
            }
            Err(problem) => {
                link.problem = Some(problem);
                None
            }
        }
    }

    pub fn sync_status(&self) -> SyncStatus {
        let Some(link) = &self.sync else {
            return SyncStatus {
                location: None,
                devices: Vec::new(),
                other_journal: false,
                last_sync: None,
                problem: None,
                received: 0,
            };
        };
        let own = file_name(&link.device);
        let devices = link
            .folder
            .list()
            .map(|files| {
                files
                    .into_iter()
                    .map(|f| SyncDevice {
                        this_device: f.name == own,
                        changed_at: OffsetDateTime::from(f.modified)
                            .to_offset(self.utc_offset)
                            .into(),
                        other_journal: link.other_names().contains(&f.name),
                    })
                    .collect()
            })
            .unwrap_or_default();
        SyncStatus {
            location: Some(link.folder.label()),
            devices,
            other_journal: link.other_journal,
            last_sync: link.last_sync.map(Into::into),
            problem: link.problem,
            received: link.received,
        }
    }

    /// Joins the journal another device keeps in the sync folder (pairing,
    /// D-22). Opens its newest file with *that* journal's password, merges this
    /// journal into it and takes it over: from now on both devices have the same
    /// journal, password and recovery key. This journal is kept as an automatic
    /// backup first; fingerprint unlock has to be set up again.
    pub fn sync_join(&mut self, password: &str, now: Instant) -> AppResult<SyncStatus> {
        self.throttle.check(now).map_err(AppError::throttled)?;
        let at = self.now();
        let ours = *self.journal()?.vault().vault_id();
        let link = self
            .sync
            .as_ref()
            .ok_or_else(|| AppError::invalid_state("sync is off"))?;
        let (device, location) = (link.device.clone(), link.location.clone());

        // The other journal's newest file.
        let mut newest: Option<(SystemTime, Backup)> = None;
        for f in link
            .folder
            .list()
            .map_err(|_| sync_error("folder_unavailable"))?
        {
            if f.name == file_name(&device)
                || newest.as_ref().is_some_and(|(t, _)| *t >= f.modified)
            {
                continue;
            }
            let Ok(bytes) = link.folder.read(&f.name) else {
                continue;
            };
            if let Ok(backup) = Backup::from_bytes(&bytes)
                && *backup.vault_id() != ours
            {
                newest = Some((f.modified, backup));
            }
        }
        let (_, theirs) = newest.ok_or_else(|| sync_error("no_other_journal"))?;
        let theirs = match theirs.open_with_password(password) {
            Ok(opened) => opened,
            Err(e @ BackupError::WrongPassword) => {
                let wait = self.throttle.record_failure(now);
                return Err(AppError::from(e).with_retry(wait));
            }
            Err(e) => return Err(e.into()),
        };
        self.throttle.record_success();
        let journal = self.journal()?;

        // This journal: kept as an automatic backup, and its content decrypted
        // with its own key to be merged into the other one.
        let own_backup = journal.vault().export_backup()?;
        auto::write(&self.auto_dir(), &own_backup)?;
        let own = journal.vault().open_sync_file(own_backup)?;
        let own_records = own.decrypt()?;

        let joining = sidecar(&self.vault_path, ".joining");
        remove_vault_files(&joining)?;
        let mut joined = Journal::open(theirs.restore(&joining)?);
        joined.merge_from_device(
            own_records,
            &|id| own.open_media(id.as_bytes()).ok().flatten(),
            at,
        )?;
        joined
            .vault_mut()
            .set_local(Local::DeviceId, Some(&device))?;
        joined
            .vault_mut()
            .set_local(Local::SyncFolder, Some(&location))?;
        joined.lock()?;

        // Swap: this journal's file goes (it is in the automatic backup), the
        // joined one takes its place. Its fingerprint key opened the old key.
        let VaultState::Unlocked(old) = std::mem::replace(&mut self.state, VaultState::Locked)
        else {
            unreachable!("journal() succeeded above");
        };
        old.lock()?;
        remove_vault_files(&self.vault_path)?;
        let _ = std::fs::remove_file(DeviceKeyFile::path_for(&self.vault_path));
        std::fs::rename(&joining, &self.vault_path).map_err(|_| AppError::from(VaultError::Io))?;
        remove_vault_files(&joining)?;

        let v = vault::unlock(&self.vault_path, password)?;
        self.state = VaultState::Unlocked(Box::new(Journal::open(v)));
        if let Some(link) = &mut self.sync {
            link.written = None;
            link.seen.clear();
        }
        self.sync_now()
    }

    // ---- journal access ----

    pub fn journal(&self) -> AppResult<&Journal> {
        match &self.state {
            VaultState::Unlocked(j) => Ok(j),
            _ => Err(AppError::locked()),
        }
    }

    pub fn journal_mut(&mut self) -> AppResult<&mut Journal> {
        match &mut self.state {
            VaultState::Unlocked(j) => Ok(j),
            _ => Err(AppError::locked()),
        }
    }

    // ---- backups ----

    /// Keeps a parsed backup until the user enters its password.
    pub fn set_pending_backup(&mut self, backup: Backup) -> BackupSummary {
        let same_vault = self
            .journal()
            .is_ok_and(|j| j.vault().vault_id() == backup.vault_id());
        let summary = BackupSummary {
            records: backup.records.len(),
            same_vault,
        };
        self.pending_backup = Some(backup);
        summary
    }

    /// Opens the picked backup and restores it (no vault yet) or imports it
    /// into the unlocked journal. On a wrong secret the backup stays picked.
    pub fn import_pending(
        &mut self,
        secret: &str,
        kind: BackupSecret,
        mode: ImportMode,
        now: Instant,
    ) -> AppResult<ImportOutcome> {
        if matches!(self.state, VaultState::Locked) {
            return Err(AppError::locked());
        }
        let backup = self
            .pending_backup
            .take()
            .ok_or_else(|| AppError::invalid_state("no backup picked"))?;
        let opened = match kind {
            BackupSecret::Password => backup.clone().open_with_password(secret),
            BackupSecret::RecoveryPhrase => backup.clone().open_with_recovery(secret),
        };
        let opened = match opened {
            Ok(opened) => opened,
            Err(e) => {
                self.pending_backup = Some(backup);
                return Err(e.into());
            }
        };
        let at = self.now();
        match &mut self.state {
            VaultState::Unlocked(journal) => {
                let voice = |id: &mossbook_core::domain::Id| {
                    opened.open_media(id.as_bytes()).ok().flatten()
                };
                let report = journal.import(opened.decrypt()?, &voice, mode, at)?;
                Ok(ImportOutcome {
                    restored: false,
                    report,
                })
            }
            _ => {
                let added = opened.record_count();
                let vault = opened.restore(&self.vault_path)?;
                self.state = VaultState::Unlocked(Box::new(Journal::open(vault)));
                self.last_activity = now;
                Ok(ImportOutcome {
                    restored: true,
                    report: ImportReport {
                        added,
                        ..ImportReport::default()
                    },
                })
            }
        }
    }

    // ---- auto-lock ----

    /// The UI reports user activity (input, navigation).
    pub fn touch(&mut self, now: Instant) {
        self.last_activity = now;
    }

    fn autolock_policy(&self) -> Option<AutoLock> {
        self.journal().ok().map(|j| j.settings().autolock)
    }

    /// The app went to the background (mobile) or was minimized (desktop).
    /// Locks immediately under the default policy, unless a system dialog we
    /// opened ourselves is the reason (D-6). While a voice note is recorded the
    /// lock waits up to [`RECORDING_GRACE`] for it to be stored (D-25); the
    /// caller then runs [`Self::on_tick`] after that time.
    pub fn on_background(&mut self, now: Instant) -> AppResult<Option<LockReason>> {
        if self.dialogs_open > 0 || self.autolock_policy() != Some(AutoLock::Immediately) {
            return Ok(None);
        }
        if self.is_recording(now) {
            self.deferred_lock.get_or_insert(now + RECORDING_GRACE);
            return Ok(None);
        }
        Ok(self.lock()?.then_some(LockReason::Background))
    }

    /// Called periodically. Locks after the inactivity timeout of the policy,
    /// or once a background lock waited long enough for a recording (D-25).
    pub fn on_tick(&mut self, now: Instant) -> AppResult<Option<LockReason>> {
        if self.deferred_lock.is_some_and(|at| now >= at) {
            return Ok(self.lock()?.then_some(LockReason::Background));
        }
        let Some(policy) = self.autolock_policy() else {
            return Ok(None);
        };
        // Someone speaking does not touch the screen.
        if self.is_recording(now) {
            return Ok(None);
        }
        let timeout = policy.timeout().unwrap_or(IMMEDIATE_IDLE_TIMEOUT);
        if now.saturating_duration_since(self.last_activity) >= timeout && self.lock()? {
            return Ok(Some(LockReason::Idle));
        }
        Ok(None)
    }

    /// A background lock is waiting for a recording (D-25).
    pub fn lock_pending(&self) -> bool {
        self.deferred_lock.is_some()
    }

    fn is_recording(&self, now: Instant) -> bool {
        self.recording_since
            .is_some_and(|since| now.saturating_duration_since(since) < RECORDING_SPAN)
    }

    /// The UI started or finished recording a voice note (D-25). It reports
    /// the end only once the recording is stored and the form kept, so a lock
    /// that waited for it happens now.
    pub fn set_recording(&mut self, active: bool, now: Instant) -> AppResult<Option<LockReason>> {
        self.last_activity = now;
        if active {
            self.journal()?;
            self.recording_since = Some(now);
            return Ok(None);
        }
        self.recording_since = None;
        if self.deferred_lock.take().is_some() {
            return Ok(self.lock()?.then_some(LockReason::Background));
        }
        Ok(None)
    }

    /// Marks a system dialog as open; pair with [`Self::end_dialog`].
    pub fn begin_dialog(&mut self) {
        self.dialogs_open += 1;
    }

    pub fn end_dialog(&mut self, now: Instant) {
        self.dialogs_open = self.dialogs_open.saturating_sub(1);
        self.last_activity = now;
    }
}

/// Writes an automatic backup of `journal`; `false` if it failed. Best effort:
/// a failure must never stop the journal from locking.
fn write_auto_backup(dir: &Path, journal: &Journal) -> bool {
    journal
        .vault()
        .export_backup()
        .and_then(|backup| auto::write(dir, &backup))
        .is_ok()
}

impl SyncLink {
    fn new(folder: Box<dyn Folder>, location: String, device: String) -> Self {
        Self {
            folder,
            location,
            device,
            written: None,
            seen: HashMap::new(),
            last_sync: None,
            problem: None,
            other_journal: false,
            received: 0,
        }
    }

    /// Files found to belong to another journal (never marked as seen).
    fn other_names(&self) -> Vec<String> {
        if !self.other_journal {
            return Vec::new();
        }
        let own = file_name(&self.device);
        self.folder
            .list()
            .map(|files| {
                files
                    .into_iter()
                    .filter(|f| f.name != own && !self.seen.contains_key(&f.name))
                    .map(|f| f.name)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Merges every other device's file that changed since it was last merged.
fn receive(
    link: &mut SyncLink,
    journal: &mut Journal,
    now: OffsetDateTime,
) -> Result<SyncReport, &'static str> {
    let own = file_name(&link.device);
    let files = link.folder.list().map_err(|_| "folder_unavailable")?;
    let mut total = SyncReport::default();
    let mut damaged = false;
    link.other_journal = false;
    for f in files {
        let stamp = (f.modified, f.len);
        if f.name == own || link.seen.get(&f.name) == Some(&stamp) {
            continue;
        }
        let bytes = link
            .folder
            .read(&f.name)
            .map_err(|_| "folder_unavailable")?;
        let opened =
            match Backup::from_bytes(&bytes).and_then(|b| journal.vault().open_sync_file(b)) {
                Ok(opened) => opened,
                Err(BackupError::OtherJournal) => {
                    link.other_journal = true;
                    continue;
                }
                Err(_) => {
                    // Possibly still being written by Syncthing; tried again later.
                    damaged = true;
                    continue;
                }
            };
        let _ = journal.vault_mut().adopt_password_from(&opened);
        let Ok(records) = opened.decrypt() else {
            damaged = true;
            continue;
        };
        let report = journal
            .merge_from_device(
                records,
                &|id| opened.open_media(id.as_bytes()).ok().flatten(),
                now,
            )
            .map_err(|_| "merge_failed")?;
        total.changed += report.changed;
        total.deleted += report.deleted;
        total.skipped += report.skipped;
        link.seen.insert(f.name, stamp);
    }
    if damaged {
        return Err("damaged_file");
    }
    Ok(total)
}

/// Writes this device's file, unless the journal is as it was last written.
fn send(link: &mut SyncLink, journal: &Journal) -> Result<(), &'static str> {
    let backup = journal
        .vault()
        .export_backup()
        .map_err(|_| "write_failed")?;
    let fingerprint = backup.fingerprint();
    if link.written == Some(fingerprint) {
        return Ok(());
    }
    let bytes = backup.to_bytes().map_err(|_| "write_failed")?;
    link.folder
        .write(&file_name(&link.device), &bytes)
        .map_err(|_| "folder_unavailable")?;
    link.written = Some(fingerprint);
    Ok(())
}

fn sync_error(code: &'static str) -> AppError {
    AppError::new(code, "sync")
}

/// A random id for this device's sync file name (16 bytes, hex).
fn new_device_id() -> AppResult<String> {
    let id = mossbook_core::domain::Id::new_random().map_err(AppError::from)?;
    Ok(id.as_bytes().iter().map(|b| format!("{b:02x}")).collect())
}

/// `vault.db` → `vault.db<suffix>`.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Removes a vault file and its SQLite companions, if present.
fn remove_vault_files(path: &Path) -> AppResult<()> {
    for file in [
        path.to_path_buf(),
        sidecar(path, "-wal"),
        sidecar(path, "-shm"),
    ] {
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(VaultError::Io.into());
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mossbook_core::domain::Settings;
    use tempfile::TempDir;

    const PASSWORD: &str = "correct horse battery";

    fn session() -> (TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let s = Session::with_fast_kdf(dir.path().join("vault.db"));
        (dir, s)
    }

    fn answers(setup: &RecoverySetup) -> Vec<RecoveryAnswer> {
        setup
            .positions
            .iter()
            .map(|&p| RecoveryAnswer {
                position: p,
                word: setup.words[p].clone(),
            })
            .collect()
    }

    fn unlocked() -> (TempDir, Session) {
        let (dir, mut s) = session();
        let setup = s.create(PASSWORD).unwrap();
        s.confirm_recovery(&answers(&setup), Instant::now())
            .unwrap();
        (dir, s)
    }

    fn state(s: &Session) -> StatusKind {
        s.status(Instant::now()).state
    }

    #[test]
    fn create_confirm_lock_unlock() {
        let (dir, mut s) = session();
        assert_eq!(state(&s), StatusKind::NoVault);
        let setup = s.create(PASSWORD).unwrap();
        assert_eq!(setup.words.len(), 24);
        assert_eq!(state(&s), StatusKind::Creating);
        assert!(!dir.path().join("vault.db").exists());

        let mut wrong = answers(&setup);
        wrong[0].word = "zzz".into();
        assert_eq!(
            s.confirm_recovery(&wrong, Instant::now()).unwrap_err().code,
            "recovery_mismatch"
        );
        assert_eq!(state(&s), StatusKind::Creating);

        s.confirm_recovery(&answers(&setup), Instant::now())
            .unwrap();
        assert_eq!(state(&s), StatusKind::Unlocked);
        assert!(s.lock().unwrap());
        assert!(!s.lock().unwrap());
        assert_eq!(state(&s), StatusKind::Locked);
        assert_eq!(s.journal().unwrap_err().code, "locked");

        s.unlock(PASSWORD, Instant::now()).unwrap();
        assert_eq!(state(&s), StatusKind::Unlocked);
        assert_eq!(
            s.unlock(PASSWORD, Instant::now()).unwrap_err().code,
            "invalid_state"
        );
        // A new session on the same file starts locked.
        assert_eq!(
            state(&Session::new(dir.path().join("vault.db"))),
            StatusKind::Locked
        );
    }

    /// Fingerprint unlock with the platform stood in for: the sealed key is
    /// kept as is, since only the platform can open it.
    fn enable_device_unlock(s: &Session) -> Vec<u8> {
        let enrollment = s.device_enrollment(PASSWORD).unwrap();
        let key = enrollment.key_bytes().to_vec();
        let file = enrollment.finish(vec![0x5a; 60]).unwrap();
        s.save_device_key(&file).unwrap();
        key
    }

    #[test]
    fn device_unlock_needs_the_password_and_opens_the_journal() {
        let (_dir, mut s) = unlocked();
        assert!(!s.device_unlock_enabled());
        assert_eq!(
            s.device_enrollment("wrong password").unwrap_err().code,
            "wrong_password"
        );
        let key = enable_device_unlock(&s);
        assert!(s.device_unlock_enabled());
        // Only a locked journal is opened with the device key.
        assert_eq!(s.device_key_file().unwrap_err().code, "invalid_state");

        s.lock().unwrap();
        let file = s.device_key_file().unwrap();
        let mut released = key.clone();
        s.unlock_with_device_key(&file, &mut released, Instant::now())
            .unwrap();
        assert_eq!(state(&s), StatusKind::Unlocked);
        assert!(released.iter().all(|&b| b == 0));
    }

    #[test]
    fn a_rejected_device_key_turns_fingerprint_unlock_off() {
        let (_dir, mut s) = unlocked();
        let mut key = enable_device_unlock(&s);
        s.lock().unwrap();
        let file = s.device_key_file().unwrap();
        key[0] ^= 1;
        assert_eq!(
            s.unlock_with_device_key(&file, &mut key, Instant::now())
                .unwrap_err()
                .code,
            "device_key_rejected"
        );
        assert!(!s.device_unlock_enabled());
        assert_eq!(state(&s), StatusKind::Locked);
        assert_eq!(s.device_key_file().unwrap_err().code, "device_unlock_off");
        s.unlock(PASSWORD, Instant::now()).unwrap();
    }

    #[test]
    fn a_damaged_device_key_file_is_removed() {
        let (dir, mut s) = unlocked();
        enable_device_unlock(&s);
        s.lock().unwrap();
        std::fs::write(dir.path().join("vault.db.device-key"), b"damaged").unwrap();
        assert_eq!(s.device_key_file().unwrap_err().code, "device_unlock_off");
        assert!(!s.device_unlock_enabled());
    }

    #[test]
    fn turning_device_unlock_off_and_destroying_remove_the_file() {
        let (dir, mut s) = unlocked();
        enable_device_unlock(&s);
        s.forget_device_key().unwrap();
        assert!(!s.device_unlock_enabled());
        enable_device_unlock(&s);
        s.destroy(PASSWORD).unwrap();
        assert!(!dir.path().join("vault.db.device-key").exists());
    }

    #[test]
    fn cancel_and_restart_creation() {
        let (_dir, mut s) = session();
        let first = s.create(PASSWORD).unwrap();
        let second = s.create(PASSWORD).unwrap();
        assert_ne!(first.words, second.words);
        s.cancel_create();
        assert_eq!(state(&s), StatusKind::NoVault);
        assert_eq!(
            s.confirm_recovery(&answers(&second), Instant::now())
                .unwrap_err()
                .code,
            "invalid_state"
        );
    }

    #[test]
    fn wrong_password_is_throttled() {
        let (_dir, mut s) = unlocked();
        s.lock().unwrap();
        let t0 = Instant::now();
        let e = s.unlock("wrong password", t0).unwrap_err();
        assert_eq!((e.code, e.retry_after_ms), ("wrong_password", Some(1000)));
        // During the delay even the right password is refused without trying.
        assert_eq!(s.unlock(PASSWORD, t0).unwrap_err().code, "throttled");
        assert!(s.status(t0).retry_after_ms.is_some());
        let e = s
            .unlock("wrong again", t0 + Duration::from_secs(1))
            .unwrap_err();
        assert_eq!(e.retry_after_ms, Some(2000));
        s.unlock(PASSWORD, t0 + Duration::from_secs(3)).unwrap();
        // Success resets the delay.
        s.lock().unwrap();
        assert_eq!(
            s.unlock("wrong", t0 + Duration::from_secs(4))
                .unwrap_err()
                .retry_after_ms,
            Some(1000)
        );
    }

    #[test]
    fn background_lock_respects_policy_and_dialogs() {
        let (_dir, mut s) = unlocked();
        s.begin_dialog();
        assert_eq!(
            s.on_background(Instant::now()).unwrap(),
            None,
            "own file dialog must not lock"
        );
        s.end_dialog(Instant::now());
        assert_eq!(
            s.on_background(Instant::now()).unwrap(),
            Some(LockReason::Background)
        );
        assert_eq!(state(&s), StatusKind::Locked);

        s.unlock(PASSWORD, Instant::now()).unwrap();
        let mut settings = Settings::default();
        settings.autolock = AutoLock::After5Min;
        let now = s.now();
        s.journal_mut()
            .unwrap()
            .save_settings(settings, now)
            .unwrap();
        assert_eq!(s.on_background(Instant::now()).unwrap(), None);
        assert_eq!(state(&s), StatusKind::Unlocked);
    }

    #[test]
    fn idle_timer_locks_and_dialogs_do_not_stop_it() {
        let (_dir, mut s) = unlocked();
        let t0 = Instant::now();
        s.touch(t0);
        s.begin_dialog();
        assert_eq!(
            s.on_tick(t0 + IMMEDIATE_IDLE_TIMEOUT - Duration::from_secs(1))
                .unwrap(),
            None
        );
        assert_eq!(
            s.on_tick(t0 + IMMEDIATE_IDLE_TIMEOUT).unwrap(),
            Some(LockReason::Idle)
        );
        assert_eq!(
            s.on_tick(t0 + IMMEDIATE_IDLE_TIMEOUT * 2).unwrap(),
            None,
            "already locked"
        );
    }

    #[test]
    fn recording_keeps_the_idle_timer_away_for_at_most_its_span() {
        let (_dir, mut s) = unlocked();
        let t0 = Instant::now();
        s.set_recording(true, t0).unwrap();
        assert_eq!(s.on_tick(t0 + IMMEDIATE_IDLE_TIMEOUT).unwrap(), None);
        // A flag the UI never cleared stops counting.
        assert_eq!(
            s.on_tick(t0 + RECORDING_SPAN).unwrap(),
            Some(LockReason::Idle)
        );

        s.unlock(PASSWORD, t0).unwrap();
        s.set_recording(true, t0).unwrap();
        s.set_recording(false, t0).unwrap();
        assert_eq!(
            s.on_tick(t0 + IMMEDIATE_IDLE_TIMEOUT).unwrap(),
            Some(LockReason::Idle),
            "after recording the timer counts again"
        );
    }

    #[test]
    fn background_lock_waits_for_a_recording_to_be_stored() {
        let (_dir, mut s) = unlocked();
        let t0 = Instant::now();
        s.set_recording(true, t0).unwrap();
        assert_eq!(s.on_background(t0).unwrap(), None);
        assert_eq!(state(&s), StatusKind::Unlocked);
        // Going to the background again does not move the deadline.
        assert_eq!(s.on_background(t0 + Duration::from_secs(2)).unwrap(), None);
        // The UI stored the recording and kept the form: the lock happens now.
        assert_eq!(
            s.set_recording(false, t0 + Duration::from_secs(1)).unwrap(),
            Some(LockReason::Background)
        );
        assert_eq!(state(&s), StatusKind::Locked);
        assert_eq!(
            s.on_tick(t0 + RECORDING_GRACE).unwrap(),
            None,
            "already locked"
        );
    }

    #[test]
    fn background_lock_happens_after_the_grace_even_without_the_ui() {
        let (_dir, mut s) = unlocked();
        let t0 = Instant::now();
        s.set_recording(true, t0).unwrap();
        s.on_background(t0).unwrap();
        assert_eq!(
            s.on_tick(t0 + RECORDING_GRACE - Duration::from_millis(1))
                .unwrap(),
            None
        );
        assert_eq!(
            s.on_tick(t0 + RECORDING_GRACE).unwrap(),
            Some(LockReason::Background)
        );
        assert_eq!(state(&s), StatusKind::Locked);

        // Nothing carries over to the next unlock.
        s.unlock(PASSWORD, t0).unwrap();
        assert_eq!(s.set_recording(false, t0).unwrap(), None);
        assert_eq!(state(&s), StatusKind::Unlocked);
        assert_eq!(
            s.on_background(t0).unwrap(),
            Some(LockReason::Background),
            "no recording: locks at once"
        );
    }

    #[test]
    fn recording_needs_an_open_journal_and_other_policies_are_unchanged() {
        let (_dir, mut s) = unlocked();
        let t0 = Instant::now();
        s.lock().unwrap();
        assert!(s.set_recording(true, t0).is_err());
        assert_eq!(s.set_recording(false, t0).unwrap(), None);

        s.unlock(PASSWORD, t0).unwrap();
        let mut settings = Settings::default();
        settings.autolock = AutoLock::After5Min;
        let now = s.now();
        s.journal_mut()
            .unwrap()
            .save_settings(settings, now)
            .unwrap();
        s.set_recording(true, t0).unwrap();
        assert_eq!(s.on_background(t0).unwrap(), None);
        assert_eq!(
            s.set_recording(false, t0).unwrap(),
            None,
            "no lock was waiting"
        );
        assert_eq!(state(&s), StatusKind::Unlocked);
    }

    #[test]
    fn creation_survives_lock_requests() {
        let (_dir, mut s) = session();
        s.create(PASSWORD).unwrap();
        assert!(!s.lock().unwrap());
        assert_eq!(s.on_background(Instant::now()).unwrap(), None);
        assert_eq!(state(&s), StatusKind::Creating);
    }

    #[test]
    fn destroy_keeps_vault_on_wrong_password() {
        let (dir, mut s) = unlocked();
        let now = s.now();
        s.journal_mut()
            .unwrap()
            .quick_entry(EntryKindTag::Note, None, now)
            .unwrap();
        assert_eq!(
            s.destroy("wrong password").unwrap_err().code,
            "wrong_password"
        );
        assert_eq!(state(&s), StatusKind::Unlocked);
        assert_eq!(s.journal().unwrap().entries(&Default::default()).len(), 1);
        s.destroy(PASSWORD).unwrap();
        assert_eq!(state(&s), StatusKind::NoVault);
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn restore_and_import_a_picked_backup() {
        let (_src_dir, mut src) = unlocked();
        let now = src.now();
        src.journal_mut()
            .unwrap()
            .quick_entry(EntryKindTag::Note, None, now)
            .unwrap();
        let backup = src.journal().unwrap().vault().export_backup().unwrap();

        // No vault yet: the backup becomes the vault (keeps its password).
        let (_dir, mut s) = session();
        assert!(!s.set_pending_backup(backup.clone()).same_vault);
        let e = s
            .import_pending(
                "wrong",
                BackupSecret::Password,
                ImportMode::Merge,
                Instant::now(),
            )
            .unwrap_err();
        assert_eq!(e.code, "wrong_password");
        let outcome = s
            .import_pending(
                PASSWORD,
                BackupSecret::Password,
                ImportMode::Merge,
                Instant::now(),
            )
            .unwrap();
        assert!(outcome.restored);
        assert_eq!(state(&s), StatusKind::Unlocked);
        assert_eq!(s.journal().unwrap().entries(&Default::default()).len(), 1);

        // Into the same, unlocked vault: nothing new.
        assert!(s.set_pending_backup(backup).same_vault);
        let outcome = s
            .import_pending(
                PASSWORD,
                BackupSecret::Password,
                ImportMode::Merge,
                Instant::now(),
            )
            .unwrap();
        assert!(!outcome.restored);
        assert_eq!(outcome.report.added, 0);
        assert_eq!(
            s.import_pending(
                PASSWORD,
                BackupSecret::Password,
                ImportMode::Merge,
                Instant::now()
            )
            .unwrap_err()
            .code,
            "invalid_state"
        );

        // A locked vault must be unlocked first.
        s.lock().unwrap();
        let backup = src.journal().unwrap().vault().export_backup().unwrap();
        s.set_pending_backup(backup);
        assert_eq!(
            s.import_pending(
                PASSWORD,
                BackupSecret::Password,
                ImportMode::Merge,
                Instant::now()
            )
            .unwrap_err()
            .code,
            "locked"
        );
    }

    #[test]
    fn quick_entry_from_the_lock_screen() {
        let (_dir, mut s) = unlocked();
        assert_eq!(
            s.quick_entry_locked(EntryKindTag::Fear, Some(6))
                .unwrap_err()
                .code,
            "invalid_state"
        );
        s.lock().unwrap();
        s.quick_entry_locked(EntryKindTag::Fear, Some(6)).unwrap();
        assert_eq!(
            s.quick_entry_locked(EntryKindTag::Mood, Some(9))
                .unwrap_err()
                .code,
            "invalid_field"
        );
        s.unlock(PASSWORD, Instant::now()).unwrap();
        let entries = s.journal().unwrap().entries(&Default::default());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].incomplete);
    }

    fn auto_files(dir: &TempDir) -> Vec<PathBuf> {
        auto::list(&auto::dir_for(&dir.path().join("vault.db")))
            .into_iter()
            .map(|b| b.path)
            .collect()
    }

    #[test]
    fn lock_writes_an_automatic_backup_once_per_interval() {
        let (dir, mut s) = unlocked();
        let now = s.now();
        s.journal_mut()
            .unwrap()
            .quick_entry(EntryKindTag::Note, None, now)
            .unwrap();
        s.lock().unwrap();
        let files = auto_files(&dir);
        assert_eq!(files.len(), 1);
        // A real backup that opens with the password and holds the entry.
        let backup = Backup::from_bytes(&std::fs::read(&files[0]).unwrap()).unwrap();
        let opened = backup.open_with_password(PASSWORD).unwrap();
        assert_eq!(opened.record_count(), 1, "the entry");

        // Not due again right away.
        s.unlock(PASSWORD, Instant::now()).unwrap();
        s.lock().unwrap();
        assert_eq!(auto_files(&dir).len(), 1);

        let info = {
            s.unlock(PASSWORD, Instant::now()).unwrap();
            s.backup_info().unwrap()
        };
        assert!(info.auto_enabled && !info.auto_failed);
        assert_eq!(info.auto_count, 1);
        assert!(info.auto_latest.is_some());
        assert_eq!(info.last_export, None);
    }

    #[test]
    fn automatic_backups_can_be_turned_off() {
        let (dir, mut s) = unlocked();
        let mut settings = Settings::default();
        settings.auto_backup = false;
        let now = s.now();
        s.journal_mut()
            .unwrap()
            .save_settings(settings, now)
            .unwrap();
        s.lock().unwrap();
        assert!(auto_files(&dir).is_empty());
    }

    #[test]
    fn password_change_replaces_automatic_backups() {
        let (dir, mut s) = unlocked();
        s.lock().unwrap();
        s.unlock(PASSWORD, Instant::now()).unwrap();
        s.change_password(PASSWORD, "a new synthetic password")
            .unwrap();
        let files = auto_files(&dir);
        assert_eq!(files.len(), 1);
        let backup = Backup::from_bytes(&std::fs::read(&files[0]).unwrap()).unwrap();
        assert!(backup.clone().open_with_password(PASSWORD).is_err());
        assert!(
            backup
                .open_with_password("a new synthetic password")
                .is_ok()
        );
    }

    #[test]
    fn destroy_removes_automatic_backups() {
        let (dir, mut s) = unlocked();
        s.lock().unwrap();
        assert_eq!(auto_files(&dir).len(), 1);
        s.unlock(PASSWORD, Instant::now()).unwrap();
        s.destroy(PASSWORD).unwrap();
        assert!(auto_files(&dir).is_empty());
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn recovery_phrase_can_be_checked() {
        let (_dir, mut s) = session();
        let setup = s.create(PASSWORD).unwrap();
        s.confirm_recovery(&answers(&setup), Instant::now())
            .unwrap();
        s.verify_recovery(&setup.words.join(" ")).unwrap();
        // Swap two different words (two equal neighbours would change nothing).
        let mut other = setup.words.clone();
        let i = (0..other.len() - 1)
            .find(|&i| other[i] != other[i + 1])
            .unwrap();
        other.swap(i, i + 1);
        assert!(s.verify_recovery(&other.join(" ")).is_err());
        s.lock().unwrap();
        assert_eq!(
            s.verify_recovery(&setup.words.join(" ")).unwrap_err().code,
            "locked"
        );
    }

    // ---- sync (D-22) ----

    fn unlocked_with(password: &str) -> (TempDir, Session) {
        let (dir, mut s) = session();
        let setup = s.create(password).unwrap();
        s.confirm_recovery(&answers(&setup), Instant::now())
            .unwrap();
        (dir, s)
    }

    fn folder(dir: &TempDir) -> Box<dyn Folder> {
        Box::new(crate::sync::DirFolder(dir.path().to_path_buf()))
    }

    fn notes(s: &Session) -> Vec<String> {
        let mut notes: Vec<String> = s
            .journal()
            .unwrap()
            .entries(&Default::default())
            .into_iter()
            .map(|e| e.note.clone())
            .collect();
        notes.sort();
        notes
    }

    fn add_note(s: &mut Session, text: &str) {
        let now = s.now();
        let mut e =
            mossbook_core::domain::Entry::new(mossbook_core::domain::EntryKind::Note, now).unwrap();
        e.note = text.into();
        s.journal_mut().unwrap().save_entry(e, now).unwrap();
    }

    #[test]
    fn pairing_joins_the_other_journal_and_keeps_this_one_as_a_backup() {
        let shared = tempfile::tempdir().unwrap();
        let (_a_dir, mut a) = unlocked_with("phone password");
        add_note(&mut a, "from the phone");
        a.sync_connect(folder(&shared), "shared".into()).unwrap();

        let (b_dir, mut b) = unlocked_with("computer password");
        add_note(&mut b, "from the computer");
        let status = b.sync_connect(folder(&shared), "shared".into()).unwrap();
        assert!(status.other_journal, "offered to join the phone's journal");
        assert_eq!(status.devices.len(), 2);

        // The wrong password changes nothing.
        let err = b
            .sync_join("computer password", Instant::now())
            .unwrap_err();
        assert_eq!(err.code, "wrong_password");
        assert_eq!(notes(&b), vec!["from the computer"]);

        b.throttle = UnlockThrottle::new();
        let status = b.sync_join("phone password", Instant::now()).unwrap();
        assert!(!status.other_journal);
        assert_eq!(notes(&b), vec!["from the computer", "from the phone"]);
        // The joined journal opens with the phone's password, here and after a restart.
        b.lock().unwrap();
        assert!(b.unlock("computer password", Instant::now()).is_err());
        b.throttle = UnlockThrottle::new();
        b.unlock("phone password", Instant::now()).unwrap();
        // The computer's own journal is kept as an automatic backup.
        let kept = auto_files(&b_dir)
            .into_iter()
            .map(|p| Backup::from_bytes(&std::fs::read(p).unwrap()).unwrap())
            .any(|backup| backup.open_with_password("computer password").is_ok());
        assert!(kept);

        // The phone receives the computer's entries.
        let status = a.sync_now().unwrap();
        assert_eq!(status.problem, None);
        assert_eq!(notes(&a), vec!["from the computer", "from the phone"]);
    }

    #[test]
    fn changes_are_written_on_lock_and_read_on_unlock() {
        let shared = tempfile::tempdir().unwrap();
        let (_a_dir, mut a) = unlocked_with(PASSWORD);
        a.sync_connect(folder(&shared), "shared".into()).unwrap();
        let (_b_dir, mut b) = unlocked_with("another password");
        b.sync_connect(folder(&shared), "shared".into()).unwrap();
        b.sync_join(PASSWORD, Instant::now()).unwrap();
        b.lock().unwrap();

        add_note(&mut a, "written while the other was locked");
        a.lock().unwrap();
        b.unlock(PASSWORD, Instant::now()).unwrap();
        assert_eq!(notes(&b), vec!["written while the other was locked"]);

        // The received entry is written into this device's file once; after
        // that, with nothing changed, locking does not rewrite it.
        b.sync_now().unwrap();
        let own = shared
            .path()
            .join(file_name(&b.sync.as_ref().unwrap().device));
        let before = std::fs::metadata(&own).unwrap().modified().unwrap();
        b.sync_now().unwrap();
        b.lock().unwrap();
        assert_eq!(std::fs::metadata(&own).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn sync_survives_a_restart_and_can_be_turned_off() {
        let shared = tempfile::tempdir().unwrap();
        let (dir, mut a) = unlocked_with(PASSWORD);
        a.sync_connect(folder(&shared), "shared".into()).unwrap();
        a.lock().unwrap();
        let path = dir.path().join("vault.db");
        assert_eq!(
            vault::read_local(&path, Local::SyncFolder)
                .unwrap()
                .as_deref(),
            Some("shared")
        );
        assert!(vault::read_local(&path, Local::DeviceId).unwrap().is_some());

        a.unlock(PASSWORD, Instant::now()).unwrap();
        a.sync_disconnect().unwrap();
        assert_eq!(a.sync_status().location, None);
        a.lock().unwrap();
        assert_eq!(vault::read_local(&path, Local::SyncFolder).unwrap(), None);
    }

    #[test]
    fn utc_offset_applies_to_now() {
        let (_dir, mut s) = session();
        s.set_utc_offset(120).unwrap();
        assert_eq!(s.now().offset().whole_minutes(), 120);
        s.set_utc_offset(-12 * 60).unwrap();
        s.set_utc_offset(14 * 60).unwrap();
        assert!(s.set_utc_offset(14 * 60 + 1).is_err());
        assert!(s.set_utc_offset(i32::MIN).is_err());
    }
}

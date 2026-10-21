//! Vault session: the state machine behind the commands and the auto-lock
//! policy (SPEC §8.3, V-3, decisions D-6 and D-11). Plain Rust, no Tauri, so
//! it can be unit-tested.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use mossbook_core::backup::{Backup, auto};
use mossbook_core::crypto::kdf::KdfParams;
use mossbook_core::domain::{AutoLock, EntryKindTag, Timestamp};
use mossbook_core::journal::{ImportMode, ImportReport, Journal};
use mossbook_core::vault::{self, DestroyError, PendingVault, UnlockThrottle, VaultError};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset};

use crate::error::{AppError, AppResult};

/// Inactivity timeout when the policy is "immediately": locking on background
/// covers phones, and on desktop (where background is fuzzy) this timer does (D-11).
pub const IMMEDIATE_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

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
    utc_offset: UtcOffset,
    /// Cheap KDF params for tests; `None` in the app (calibrated params).
    kdf_override: Option<KdfParams>,
    /// A backup file picked by the user, waiting for its password (X-2).
    pending_backup: Option<Backup>,
    /// The last automatic backup that was due could not be written (D-16).
    auto_backup_failed: bool,
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
            utc_offset: UtcOffset::UTC,
            kdf_override: None,
            pending_backup: None,
            auto_backup_failed: false,
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
                // The journal is gone; its automatic backups go with it.
                auto::clear(&self.auto_dir())?;
                self.auto_backup_failed = false;
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
        match &mut self.state {
            VaultState::Unlocked(journal) => {
                let report = journal.import(opened.decrypt()?, mode)?;
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
    /// opened ourselves is the reason (D-6).
    pub fn on_background(&mut self) -> AppResult<Option<LockReason>> {
        if self.dialogs_open == 0
            && self.autolock_policy() == Some(AutoLock::Immediately)
            && self.lock()?
        {
            return Ok(Some(LockReason::Background));
        }
        Ok(None)
    }

    /// Called periodically. Locks after the inactivity timeout of the policy.
    pub fn on_tick(&mut self, now: Instant) -> AppResult<Option<LockReason>> {
        let Some(policy) = self.autolock_policy() else {
            return Ok(None);
        };
        let timeout = policy.timeout().unwrap_or(IMMEDIATE_IDLE_TIMEOUT);
        if now.saturating_duration_since(self.last_activity) >= timeout && self.lock()? {
            return Ok(Some(LockReason::Idle));
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
            s.on_background().unwrap(),
            None,
            "own file dialog must not lock"
        );
        s.end_dialog(Instant::now());
        assert_eq!(s.on_background().unwrap(), Some(LockReason::Background));
        assert_eq!(state(&s), StatusKind::Locked);

        s.unlock(PASSWORD, Instant::now()).unwrap();
        let mut settings = Settings::default();
        settings.autolock = AutoLock::After5Min;
        s.journal_mut().unwrap().save_settings(settings).unwrap();
        assert_eq!(s.on_background().unwrap(), None);
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
    fn creation_survives_lock_requests() {
        let (_dir, mut s) = session();
        s.create(PASSWORD).unwrap();
        assert!(!s.lock().unwrap());
        assert_eq!(s.on_background().unwrap(), None);
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
        s.journal_mut().unwrap().save_settings(settings).unwrap();
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
        let mut other = setup.words.clone();
        other.swap(0, 1);
        assert!(s.verify_recovery(&other.join(" ")).is_err());
        s.lock().unwrap();
        assert_eq!(
            s.verify_recovery(&setup.words.join(" ")).unwrap_err().code,
            "locked"
        );
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

//! Tauri commands: a thin mapping from IPC calls to [`Session`] and the core
//! journal (SPEC §8.3). Every command returns `Result<T, AppError>`.
//!
//! Commands run off the main thread (`async`), so Argon2id never freezes the UI.
//! Passwords and phrases arrive as `String` and are moved into `Zeroizing` at
//! once; the copies inside the webview cannot be wiped (see SECURITY.md).

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use mossbook_core::backup::{Backup, BackupError, MAX_BACKUP_BYTES};
use mossbook_core::crisis::{self, CrisisDirectory};
use mossbook_core::domain::{Entry, EntryDraft, EntryKindTag, Id, SafetyPlan, Settings, Tag};
use mossbook_core::journal::ImportMode;
use mossbook_core::query::{EntryFilter, MoodPoint, StatsRange};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
use tauri_plugin_opener::OpenerExt;
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};
use crate::session::{
    BackupSecret, BackupSummary, ImportOutcome, LockReason, RecoveryAnswer, RecoverySetup, Session,
    Status,
};

/// Event sent to the UI whenever the vault locks.
pub const EVENT_LOCKED: &str = "vault-locked";

pub struct AppState {
    session: Mutex<Session>,
}

impl AppState {
    pub fn new(session: Session) -> Self {
        Self {
            session: Mutex::new(session),
        }
    }

    /// The session; a panic in another command must not leave the vault unusable.
    pub fn session(&self) -> MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs `f` on the session and counts the call as user activity.
fn with_session<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut Session) -> AppResult<T>,
) -> AppResult<T> {
    let mut session = state.session();
    session.touch(Instant::now());
    f(&mut session)
}

pub fn emit_locked(app: &AppHandle, reason: LockReason) {
    // Nothing useful can be done if the webview is gone; the vault is locked either way.
    let _ = app.emit(EVENT_LOCKED, reason);
}

// ---- clock and activity ----

#[tauri::command(async)]
pub fn clock_sync(state: State<'_, AppState>, offset_minutes: i32) -> AppResult<()> {
    state.session().set_utc_offset(offset_minutes)
}

#[tauri::command(async)]
pub fn activity_ping(state: State<'_, AppState>) {
    state.session().touch(Instant::now());
}

// ---- vault ----

#[tauri::command(async)]
pub fn vault_status(state: State<'_, AppState>) -> Status {
    state.session().status(Instant::now())
}

#[tauri::command(async)]
pub fn vault_create(state: State<'_, AppState>, password: String) -> AppResult<RecoverySetup> {
    let password = Zeroizing::new(password);
    with_session(&state, |s| s.create(&password))
}

#[tauri::command(async)]
pub fn vault_cancel_create(state: State<'_, AppState>) {
    state.session().cancel_create();
}

#[tauri::command(async)]
pub fn vault_confirm_recovery(
    state: State<'_, AppState>,
    answers: Vec<RecoveryAnswer>,
) -> AppResult<()> {
    with_session(&state, |s| s.confirm_recovery(&answers, Instant::now()))
}

#[tauri::command(async)]
pub fn vault_unlock(state: State<'_, AppState>, password: String) -> AppResult<()> {
    let password = Zeroizing::new(password);
    with_session(&state, |s| s.unlock(&password, Instant::now()))
}

#[tauri::command(async)]
pub fn vault_unlock_recovery(
    state: State<'_, AppState>,
    phrase: String,
    new_password: String,
) -> AppResult<()> {
    let phrase = Zeroizing::new(phrase);
    let new_password = Zeroizing::new(new_password);
    with_session(&state, |s| {
        s.unlock_with_recovery(&phrase, &new_password, Instant::now())
    })
}

#[tauri::command(async)]
pub fn vault_lock(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    if state.session().lock()? {
        emit_locked(&app, LockReason::Manual);
    }
    Ok(())
}

#[tauri::command(async)]
pub fn vault_change_password(
    state: State<'_, AppState>,
    old: String,
    new: String,
) -> AppResult<()> {
    let (old, new) = (Zeroizing::new(old), Zeroizing::new(new));
    with_session(&state, |s| s.change_password(&old, &new))
}

#[tauri::command(async)]
pub fn vault_destroy(state: State<'_, AppState>, password: String) -> AppResult<()> {
    let password = Zeroizing::new(password);
    with_session(&state, |s| s.destroy(&password))
}

// ---- entries ----

#[tauri::command(async)]
pub fn entries_list(state: State<'_, AppState>, filter: EntryFilter) -> AppResult<Vec<Entry>> {
    with_session(&state, |s| {
        Ok(s.journal()?.entries(&filter).into_iter().cloned().collect())
    })
}

#[tauri::command(async)]
pub fn entry_get(state: State<'_, AppState>, id: Id) -> AppResult<Entry> {
    with_session(&state, |s| {
        s.journal()?
            .entry(&id)
            .cloned()
            .ok_or_else(|| AppError::new("not_found", "no such entry"))
    })
}

#[tauri::command(async)]
pub fn entry_save(state: State<'_, AppState>, draft: EntryDraft) -> AppResult<Entry> {
    with_session(&state, |s| {
        let now = s.now();
        Ok(s.journal_mut()?.save_draft(&draft, now)?.clone())
    })
}

#[tauri::command(async)]
pub fn entry_delete(state: State<'_, AppState>, id: Id) -> AppResult<bool> {
    with_session(&state, |s| Ok(s.journal_mut()?.delete_entry(&id)?))
}

#[tauri::command(async)]
pub fn quick_entry(
    state: State<'_, AppState>,
    kind: EntryKindTag,
    value: Option<u8>,
) -> AppResult<Entry> {
    with_session(&state, |s| {
        let now = s.now();
        Ok(s.journal_mut()?.quick_entry(kind, value, now)?.clone())
    })
}

// ---- tags ----

#[tauri::command(async)]
pub fn tags_list(state: State<'_, AppState>) -> AppResult<Vec<Tag>> {
    with_session(&state, |s| {
        Ok(s.journal()?.tags().into_iter().cloned().collect())
    })
}

#[tauri::command(async)]
pub fn tag_create(state: State<'_, AppState>, name: String, color: String) -> AppResult<Tag> {
    with_session(&state, |s| {
        let now = s.now();
        Ok(s.journal_mut()?.create_tag(&name, &color, now)?.clone())
    })
}

#[tauri::command(async)]
pub fn tag_save(state: State<'_, AppState>, tag: Tag) -> AppResult<Tag> {
    with_session(&state, |s| {
        let journal = s.journal_mut()?;
        if !journal.tags().iter().any(|t| t.id == tag.id) {
            return Err(AppError::new("not_found", "no such tag"));
        }
        Ok(journal.save_tag(tag)?.clone())
    })
}

#[tauri::command(async)]
pub fn tag_delete(state: State<'_, AppState>, id: Id) -> AppResult<bool> {
    with_session(&state, |s| {
        let now = s.now();
        Ok(s.journal_mut()?.delete_tag(&id, now)?)
    })
}

#[tauri::command(async)]
pub fn tag_merge(state: State<'_, AppState>, from: Id, into: Id) -> AppResult<()> {
    with_session(&state, |s| {
        let now = s.now();
        Ok(s.journal_mut()?.merge_tags(&from, &into, now)?)
    })
}

// ---- plan and settings ----

#[tauri::command(async)]
pub fn plan_get(state: State<'_, AppState>) -> AppResult<SafetyPlan> {
    with_session(&state, |s| Ok(s.journal()?.plan().clone()))
}

#[tauri::command(async)]
pub fn plan_save(state: State<'_, AppState>, plan: SafetyPlan) -> AppResult<SafetyPlan> {
    with_session(&state, |s| Ok(s.journal_mut()?.save_plan(plan)?.clone()))
}

#[tauri::command(async)]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<Settings> {
    with_session(&state, |s| Ok(s.journal()?.settings().clone()))
}

#[tauri::command(async)]
pub fn settings_save(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    with_session(&state, |s| {
        Ok(s.journal_mut()?.save_settings(settings)?.clone())
    })
}

// ---- patterns ----

#[tauri::command(async)]
pub fn stats_mood(state: State<'_, AppState>, range: StatsRange) -> AppResult<Vec<MoodPoint>> {
    with_session(&state, |s| {
        let now = s.now();
        Ok(s.journal()?.mood_series(range, now))
    })
}

// ---- backups (X-1, X-2) ----

/// Runs a system file dialog. The background lock is suspended meanwhile
/// (decision D-6); the session is not held while the dialog is open.
fn with_dialog<T>(state: &State<'_, AppState>, dialog: impl FnOnce() -> T) -> T {
    state.session().begin_dialog();
    let result = dialog();
    state.session().end_dialog(Instant::now());
    result
}

/// Saves an encrypted backup where the user chooses. `false` if cancelled.
#[tauri::command(async)]
pub fn backup_export(app: AppHandle, state: State<'_, AppState>) -> AppResult<bool> {
    let name = {
        let session = state.session();
        session.journal()?;
        format!("mossbook-{}.mossbook", session.now().date())
    };
    let target = with_dialog(&state, || {
        app.dialog()
            .file()
            .add_filter("Mossbook", &["mossbook"])
            .set_file_name(name)
            .blocking_save_file()
    });
    let Some(target) = target else {
        return Ok(false);
    };
    // The vault may have locked on idle while the dialog was open.
    let bytes = state
        .session()
        .journal()?
        .vault()
        .export_backup()?
        .to_bytes()?;
    write_file(&app, target, &bytes)
        .map_err(|_| AppError::new("io", "cannot write the backup file"))?;
    Ok(true)
}

fn write_file(app: &AppHandle, target: FilePath, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = app.fs().open(target, options)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Lets the user pick a backup file and checks its structure. `None` if cancelled.
#[tauri::command(async)]
pub fn backup_pick(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<BackupSummary>> {
    let picked = with_dialog(&state, || {
        app.dialog()
            .file()
            .add_filter("Mossbook", &["mossbook"])
            .blocking_pick_file()
    });
    let Some(picked) = picked else {
        return Ok(None);
    };
    let bytes = read_limited(&app, picked).map_err(|e| match e {
        ReadError::TooLarge => AppError::from(BackupError::TooLarge),
        ReadError::Io => AppError::new("io", "cannot read the file"),
    })?;
    let backup = Backup::from_bytes(&bytes)?;
    Ok(Some(state.session().set_pending_backup(backup)))
}

enum ReadError {
    TooLarge,
    Io,
}

fn read_limited(app: &AppHandle, path: FilePath) -> Result<Vec<u8>, ReadError> {
    use std::io::Read;
    let mut options = OpenOptions::new();
    options.read(true);
    let file = app.fs().open(path, options).map_err(|_| ReadError::Io)?;
    let mut bytes = Vec::new();
    file.take(MAX_BACKUP_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReadError::Io)?;
    if bytes.len() > MAX_BACKUP_BYTES {
        return Err(ReadError::TooLarge);
    }
    Ok(bytes)
}

/// Opens the picked backup with its password or recovery phrase and restores
/// it (no vault yet) or imports it into the unlocked journal.
#[tauri::command(async)]
pub fn backup_import(
    state: State<'_, AppState>,
    secret: String,
    kind: BackupSecret,
    mode: ImportMode,
) -> AppResult<ImportOutcome> {
    let secret = Zeroizing::new(secret);
    with_session(&state, |s| {
        s.import_pending(&secret, kind, mode, Instant::now())
    })
}

// ---- clipboard ----

/// Clears the clipboard if it still holds `text`, copied from the app earlier
/// (SPEC §2: copied fragments must not linger). Returns whether it cleared.
/// Anything the user copied elsewhere in the meantime is left alone.
#[tauri::command(async)]
pub fn clipboard_clear_if(app: AppHandle, text: String) -> AppResult<bool> {
    let text = Zeroizing::new(text);
    let current = Zeroizing::new(app.clipboard().read_text().unwrap_or_default());
    if !same_clipboard_text(&text, &current) {
        return Ok(false);
    }
    app.clipboard()
        .clear()
        .map_err(|_| AppError::new("internal", "clipboard unavailable"))?;
    Ok(true)
}

/// Compares ignoring line-ending style and surrounding whitespace, which
/// platforms change when text goes through the clipboard.
fn same_clipboard_text(copied: &str, current: &str) -> bool {
    let normalize = |s: &str| Zeroizing::new(s.replace("\r\n", "\n").trim().to_owned());
    let copied = normalize(copied);
    !copied.is_empty() && *copied == *normalize(current)
}

// ---- crisis lines (available without unlocking, P-3) ----

#[derive(serde::Serialize)]
pub struct CrisisInfo {
    /// Countries with crisis data, for the country picker.
    countries: Vec<&'static str>,
    /// Numbers for the requested country, if there is data for it.
    directory: Option<CrisisDirectory>,
}

/// Static crisis numbers. Needs no unlock and touches no user data.
#[tauri::command(async)]
pub fn crisis_lines(country: String) -> CrisisInfo {
    CrisisInfo {
        countries: crisis::countries(),
        directory: crisis::directory(&country),
    }
}

/// Starts a phone call. Only crisis numbers and, when unlocked, the user's own
/// safety-plan contacts can be dialed, so a compromised web view cannot call
/// arbitrary numbers.
#[tauri::command(async)]
pub fn dial(app: AppHandle, state: State<'_, AppState>, number: String) -> AppResult<()> {
    let wanted = phone_digits(&number);
    let allowed = crisis::is_known_number(&wanted)
        || state.session().journal().is_ok_and(|j| {
            let plan = j.plan();
            plan.contacts
                .iter()
                .chain(&plan.professionals)
                .any(|c| phone_digits(&c.phone) == wanted)
        });
    if wanted.is_empty() || !allowed {
        return Err(AppError::new("invalid_field", "number"));
    }
    app.opener()
        .open_url(format!("tel:{wanted}"), None::<&str>)
        .map_err(|_| AppError::new("internal", "cannot start a call"))
}

/// Keeps a leading `+` and the digits: `+48 600-000-000` → `+48600000000`.
fn phone_digits(number: &str) -> String {
    let trimmed = number.trim();
    let plus = if trimmed.starts_with('+') { "+" } else { "" };
    format!(
        "{plus}{}",
        trimmed
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::{phone_digits, same_clipboard_text};

    #[test]
    fn phone_numbers_are_normalized() {
        assert_eq!(phone_digits(" +48 600-000-000 "), "+48600000000");
        assert_eq!(phone_digits("800 70 2222"), "800702222");
        assert_eq!(phone_digits("tel:112"), "112");
        assert_eq!(phone_digits("abc"), "");
    }

    #[test]
    fn clipboard_comparison() {
        assert!(same_clipboard_text(
            "line one\nline two",
            "line one\r\nline two\n"
        ));
        assert!(!same_clipboard_text("from the app", "something else"));
        assert!(!same_clipboard_text("", ""));
        assert!(!same_clipboard_text("  ", "  "));
    }
}

//! Tauri commands: a thin mapping from IPC calls to [`Session`] and the core
//! journal (SPEC §8.3). Every command returns `Result<T, AppError>`.
//!
//! Commands run off the main thread (`async`), so Argon2id never freezes the UI.
//! Passwords and phrases arrive as `String` and are moved into `Zeroizing` at
//! once; the copies inside the webview cannot be wiped (see SECURITY.md).

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use mossbook_core::domain::{Entry, EntryDraft, EntryKindTag, Id, SafetyPlan, Settings, Tag};
use mossbook_core::query::{EntryFilter, MoodPoint, StatsRange};
use tauri::{AppHandle, Emitter, State};
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};
use crate::session::{LockReason, RecoveryAnswer, RecoverySetup, Session, Status};

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

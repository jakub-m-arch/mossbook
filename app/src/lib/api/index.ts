// Typed wrappers around Tauri `invoke()`. Components talk to the backend only
// through this module (CLAUDE.md). Every call rejects with an `AppError`.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type {
	AppError,
	AppInfo,
	BackupInfo,
	BackupSecret,
	BackupSummary,
	CrisisInfo,
	DeviceUnlockStatus,
	ImportMode,
	ImportOutcome,
	Entry,
	EntryDraft,
	EntryFilter,
	EntryKindTag,
	Id,
	LockReason,
	MoodPoint,
	Patterns,
	ReportDoc,
	RecoveryAnswer,
	PromptText,
	RecoverySetup,
	SafetyPlan,
	Settings,
	StatsRange,
	Status,
	Tag,
	SyncStatus,
	UnsavedEntry,
	VoiceNote
} from './types';
import { base64ToBytes, bytesToBase64 } from './bytes';

export type * from './types';

/** Narrows an unknown rejection to an `AppError`. */
export function isAppError(e: unknown): e is AppError {
	return typeof e === 'object' && e !== null && 'code' in e && 'message' in e;
}

// ---- clock and activity ----

/** Reports the local UTC offset; call on start and when the app comes back. */
export const clockSync = () =>
	invoke<void>('clock_sync', { offsetMinutes: -new Date().getTimezoneOffset() });

/** Resets the idle auto-lock timer (V-3). */
export const activityPing = () => invoke<void>('activity_ping');

// ---- vault ----

export const vaultStatus = () => invoke<Status>('vault_status');
export const vaultCreate = (password: string) =>
	invoke<RecoverySetup>('vault_create', { password });
export const vaultCancelCreate = () => invoke<void>('vault_cancel_create');
export const vaultConfirmRecovery = (answers: RecoveryAnswer[]) =>
	invoke<void>('vault_confirm_recovery', { answers });
export const vaultUnlock = (password: string) => invoke<void>('vault_unlock', { password });
export const vaultUnlockRecovery = (phrase: string, newPassword: string) =>
	invoke<void>('vault_unlock_recovery', { phrase, newPassword });
export const vaultLock = () => invoke<void>('vault_lock');
export const vaultChangePassword = (old: string, next: string) =>
	invoke<void>('vault_change_password', { old, new: next });
/** Checks a written-down recovery phrase without changing anything. */
export const vaultVerifyRecovery = (phrase: string) =>
	invoke<void>('vault_verify_recovery', { phrase });
export const vaultDestroy = (password: string) => invoke<void>('vault_destroy', { password });

// ---- fingerprint unlock (V-7) ----

/** Available without unlocking, for the lock screen. */
export const deviceUnlockStatus = () => invoke<DeviceUnlockStatus>('device_unlock_status');
/** Opens the journal with a fingerprint. `false` if the user closed the prompt. */
export const vaultUnlockDevice = (prompt: PromptText) =>
	invoke<boolean>('vault_unlock_device', { prompt });
/** Turns fingerprint unlock on (password and one fingerprint). `false` if cancelled. */
export const deviceUnlockEnable = (password: string, prompt: PromptText) =>
	invoke<boolean>('device_unlock_enable', { password, prompt });
export const deviceUnlockDisable = () => invoke<void>('device_unlock_disable');

/** Fires whenever the vault locks (manually, in the background or when idle). */
export const onVaultLocked = (handler: (reason: LockReason) => void): Promise<UnlistenFn> =>
	listen<LockReason>('vault-locked', (event) => handler(event.payload));

/** Desktop focus changes: `true` = cover the content (decision D-11). */
export const onPrivacyCurtain = (handler: (covered: boolean) => void): Promise<UnlistenFn> =>
	listen<boolean>('privacy-curtain', (event) => handler(event.payload));

// ---- entries ----

export const entriesList = (filter: EntryFilter = {}) =>
	invoke<Entry[]>('entries_list', { filter });
export const entryGet = (id: Id) => invoke<Entry>('entry_get', { id });
export const entrySave = (draft: EntryDraft) => invoke<Entry>('entry_save', { draft });
export const entryDelete = (id: Id) => invoke<boolean>('entry_delete', { id });

// ---- voice notes (E-18, D-21) ----

/** Stores an Ogg Opus recording made in the form; attach the result to the entry. */
export const voiceSave = (data: Uint8Array) =>
	invoke<VoiceNote>('voice_save', { data: bytesToBase64(data) });
/** One recording for playback. Keep it in memory only (an object URL, revoked after use). */
export const voiceGet = (id: Id) => invoke<string>('voice_get', { id }).then(base64ToBytes);
/** Quick entry (E-10): mood rating 1–5 for `mood`, intensity 0–10 otherwise. */
export const quickEntry = (kind: EntryKindTag, value: number | null) =>
	invoke<Entry>('quick_entry', { kind, value });
/** Quick entry while locked (E-15): sealed into the inbox, readable only after unlock. */
export const quickEntryLocked = (kind: EntryKindTag, value: number | null) =>
	invoke<void>('quick_entry_locked', { kind, value });

// ---- tags ----

export const tagsList = () => invoke<Tag[]>('tags_list');
export const tagCreate = (name: string, color: string) =>
	invoke<Tag>('tag_create', { name, color });
export const tagSave = (tag: Tag) => invoke<Tag>('tag_save', { tag });
export const tagDelete = (id: Id) => invoke<boolean>('tag_delete', { id });
export const tagMerge = (from: Id, into: Id) => invoke<void>('tag_merge', { from, into });

// ---- plan and settings ----

export const planGet = () => invoke<SafetyPlan>('plan_get');
export const planSave = (plan: SafetyPlan) => invoke<SafetyPlan>('plan_save', { plan });
export const settingsGet = () => invoke<Settings>('settings_get');
export const settingsSave = (settings: Settings) => invoke<Settings>('settings_save', { settings });

// ---- unsaved entry form (D-20) ----

export const unsavedGet = () => invoke<UnsavedEntry | null>('unsaved_get');
export const unsavedKeep = (draft: EntryDraft) => invoke<null>('unsaved_keep', { draft });
export const unsavedDiscard = () => invoke<null>('unsaved_discard');

// ---- patterns ----

export const statsMood = (range: StatsRange) => invoke<MoodPoint[]>('stats_mood', { range });
export const statsPatterns = (range: StatsRange) => invoke<Patterns>('stats_patterns', { range });

// ---- report (X-3) ----

/**
 * Renders the report to PDF and opens the save dialog. `false` if cancelled.
 * With `password`, the PDF is encrypted with AES-256.
 */
export const reportPdf = (report: ReportDoc, password?: string) =>
	invoke<boolean>('report_pdf', { report, password: password ?? null });

// ---- clipboard ----

/** Clears the clipboard if it still holds `text` (copied from the app earlier). */
export const clipboardClearIf = (text: string) => invoke<boolean>('clipboard_clear_if', { text });

// ---- crisis lines and calls (work without unlocking) ----

export const crisisLines = (country: string) => invoke<CrisisInfo>('crisis_lines', { country });
/** Starts a call. Only crisis numbers and safety-plan contacts are allowed. */
export const dial = (number: string) => invoke<void>('dial', { number });

// ---- about and updates (no network: the browser opens the releases page) ----

export const appInfo = () => invoke<AppInfo>('app_info');
export const openReleases = () => invoke<void>('open_releases');

// ---- backups (X-1, X-2) ----

/** Opens the system save dialog and writes an encrypted backup. `false` if cancelled. */
export const backupExport = () => invoke<boolean>('backup_export');
/** Opens the system file picker and checks the file. `null` if cancelled. */
export const backupPick = () => invoke<BackupSummary | null>('backup_pick');
/** Opens the picked backup and restores it (no vault) or imports it (unlocked). */
export const backupImport = (secret: string, kind: BackupSecret, mode: ImportMode) =>
	invoke<ImportOutcome>('backup_import', { secret, kind, mode });
/** Automatic backups and the last backup saved by the user (D-16). */
export const backupInfo = () => invoke<BackupInfo>('backup_info');
/** Shows the newest automatic backup in the system file manager. */
export const backupReveal = () => invoke<void>('backup_reveal');
/** Saves the whole journal as unencrypted JSON (X-4). `false` if cancelled. */
/** The readable, unencrypted copy (X-4): a ZIP with the JSON and the voice notes (D-21). */
export const exportReadable = () => invoke<boolean>('export_readable');

// ---- sync between the user's devices (D-22) ----

export const syncStatus = () => invoke<SyncStatus>('sync_status');
/** Opens the system folder picker and turns sync on; `null` if cancelled. */
export const syncChooseFolder = () => invoke<SyncStatus | null>('sync_choose_folder');
export const syncNow = () => invoke<SyncStatus>('sync_now');
/** Joins the journal another device keeps in the folder, with that journal's password. */
export const syncJoin = (password: string) => invoke<SyncStatus>('sync_join', { password });
export const syncDisconnect = () => invoke<void>('sync_disconnect');

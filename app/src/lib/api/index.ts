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
	RecoverySetup,
	SafetyPlan,
	Settings,
	StatsRange,
	Status,
	Tag
} from './types';

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

// ---- patterns ----

export const statsMood = (range: StatsRange) => invoke<MoodPoint[]>('stats_mood', { range });
export const statsPatterns = (range: StatsRange) => invoke<Patterns>('stats_patterns', { range });

// ---- report (X-3) ----

/** Renders the report to PDF and opens the save dialog. `false` if cancelled. */
export const reportPdf = (report: ReportDoc) => invoke<boolean>('report_pdf', { report });

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
export const exportJson = () => invoke<boolean>('export_json');

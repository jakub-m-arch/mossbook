// TypeScript mirror of the types that cross the IPC boundary.
// Source of truth: core/src/domain/*.rs, core/src/query/*.rs and
// app/src-tauri/src/{session,error}.rs. Keep in sync (decision D-7).

/** UUID string. */
export type Id = string;
/** RFC 3339 timestamp with offset, e.g. `2026-10-19T21:30:00+02:00`. */
export type Timestamp = string;

// ---- errors ----

export type ErrorCode =
	| 'no_vault'
	| 'vault_exists'
	| 'wrong_password'
	| 'wrong_recovery_key'
	| 'invalid_recovery_phrase'
	| 'password_too_short'
	| 'not_confirmed'
	| 'unsupported_format'
	| 'vault_corrupted'
	| 'recovery_mismatch'
	| 'throttled'
	| 'locked'
	| 'invalid_state'
	| 'invalid_field'
	| 'not_found'
	| 'duplicate_tag_name'
	| 'not_a_backup'
	| 'unsupported_backup'
	| 'backup_corrupted'
	| 'backup_too_large'
	| 'io'
	| 'inbox_unavailable'
	| 'busy'
	| 'cancelled'
	| 'transcription_off'
	| 'model_invalid'
	| 'model_load'
	| 'transcription_audio'
	| 'internal';

export interface AppError {
	code: ErrorCode;
	/** English, for developers only; the UI translates `code`. */
	message: string;
	retry_after_ms?: number;
}

// ---- vault ----

export type VaultState = 'no_vault' | 'creating' | 'locked' | 'unlocked';

export interface Status {
	state: VaultState;
	unreadable_records: number;
	retry_after_ms?: number;
}

/** Fingerprint unlock (V-7): never available on desktop. */
export interface DeviceUnlockStatus {
	/** Strong biometrics are set up on this device. */
	available: boolean;
	/** A strong sensor, but no fingerprint is enrolled in the system yet. */
	enrollable: boolean;
	/** Fingerprint unlock is turned on for this journal. */
	enabled: boolean;
}

/** Texts of the system biometric prompt, in the UI language. */
export interface PromptText {
	title: string;
	subtitle?: string | null;
	cancel: string;
}

export interface RecoverySetup {
	words: string[];
	/** 0-based positions the user has to retype. */
	positions: [number, number, number];
}

export interface RecoveryAnswer {
	position: number;
	word: string;
}

export type LockReason = 'manual' | 'background' | 'idle';

// ---- entries ----

export type EntryKindTag = 'mood' | 'event' | 'fear' | 'sleep' | 'nightmare' | 'note' | 'session';

export type Emotion =
	| 'anxiety'
	| 'sadness'
	| 'anger'
	| 'shame'
	| 'guilt'
	| 'relief'
	| 'calm'
	| 'joy'
	| 'loneliness'
	| 'numbness'
	| 'overwhelm'
	| { custom: string };

export type Symptom =
	| 'racing_heart'
	| 'shortness_of_breath'
	| 'chest_tightness'
	| 'sweating'
	| 'trembling'
	| 'nausea'
	| 'dizziness'
	| 'muscle_tension'
	| 'hot_or_cold_flushes'
	| 'tingling'
	| 'unreality'
	| { custom: string };

export type EntryKind =
	| { mood: { energy: number | null; emotions: Emotion[] } }
	| { event: { what: string; place: string; body: string; thoughts: string; helped: string } }
	| {
			fear: {
				trigger: string;
				symptoms: Symptom[];
				avoidance: boolean | null;
				strategy: string | null;
				intensity_after: number | null;
			};
	  }
	| {
			sleep: {
				bedtime: Timestamp | null;
				wake: Timestamp | null;
				quality: number | null;
				awakenings: number | null;
				nightmare: boolean;
			};
	  }
	| { nightmare: { content: string; rescripted: string; rehearsals: Timestamp[] } }
	| 'note'
	/** E-7 (D-26): after a therapy session; saving it clears the flag of `discussed`. */
	| { session: { agreed: string; todo: string; discussed: Id[] } };

export interface Entry {
	id: Id;
	schema: number;
	created_at: Timestamp;
	updated_at: Timestamp;
	occurred_at: Timestamp;
	kind: EntryKind;
	/** 1..=5 */
	mood: number | null;
	/** 0..=10 */
	intensity: number | null;
	tags: Id[];
	note: string;
	incomplete: boolean;
	sensitive: boolean;
	/** A recording attached to the entry (E-18); absent when there is none. */
	voice?: VoiceNote | null;
	/** Flagged for the next session (E-20); absent when not. */
	discuss?: boolean;
}

/** A voice note (E-18, D-21): Ogg Opus, at most 5 minutes, decrypted only for playback. */
export interface VoiceNote {
	id: Id;
	/** Taken from the file by the core, not from the form. */
	duration_ms: number;
	mime: string;
	/** Text the user kept from transcribing it (E-19, D-23); absent when there is none. */
	transcript?: string;
}

/** What the UI submits; the core sets id (for new entries), schema and timestamps. */
export interface EntryDraft {
	id?: Id | null;
	kind: EntryKind;
	occurred_at?: Timestamp;
	mood?: number | null;
	intensity?: number | null;
	tags?: Id[];
	note?: string;
	incomplete?: boolean;
	sensitive?: boolean;
	/** As returned by `voiceSave`; only its id is trusted by the core. */
	voice?: VoiceNote | null;
	discuss?: boolean;
}

export interface EntryFilter {
	kinds?: EntryKindTag[];
	tags?: Id[];
	from?: Timestamp;
	to?: Timestamp;
	text?: string;
	incomplete_only?: boolean;
	/** Only entries flagged for the next session (E-20). */
	discuss_only?: boolean;
}

// ---- tags, plan, settings ----

export interface Tag {
	id: Id;
	name: string;
	/** `#rrggbb` */
	color: string;
	/** Set by the core when it changes (sync, D-22); ignored when sent. */
	updated_at?: Timestamp | null;
}

export interface Contact {
	name: string;
	phone: string;
	note: string;
}

export interface SafetyPlan {
	warning_signs: string[];
	self_coping: string[];
	calming_people_places: string[];
	contacts: Contact[];
	professionals: Contact[];
	safe_environment: string[];
	/** Set by the core when it changes (sync, D-22); ignored when sent. */
	updated_at?: Timestamp | null;
}

export type AutoLock = 'immediately' | 'after1_min' | 'after5_min' | 'after15_min';

export interface Reminder {
	hour: number;
	minute: number;
	text: string;
	enabled: boolean;
}

export interface Settings {
	autolock: AutoLock;
	/** `null` follows the system language. */
	locale: 'pl' | 'en' | 'uk' | null;
	crisis_country: string;
	reminders: Reminder[];
	/** Automatic local backups when the journal locks (D-16). */
	auto_backup: boolean;
	/** When a backup was last saved by the user; set by Rust, ignored on save. */
	last_export: string | null;
	/** Set by the core when it changes (sync, D-22); ignored when sent. */
	updated_at?: Timestamp | null;
}

/** The entry form kept when the journal locked (D-20). */
export interface UnsavedEntry {
	draft: EntryDraft;
	kept_at: Timestamp;
}

// ---- patterns ----

export type StatsRange = 'week' | 'month' | 'three_months';

export interface MoodPoint {
	/** Calendar date, `YYYY-MM-DD`. */
	date: string;
	average: number;
	count: number;
}

/** A-2–A-4: plain counts and averages; empty lists below 14 entries. */
export interface Patterns {
	entries: number;
	enough_data: boolean;
	top_tags: PatternCount[];
	top_triggers: PatternCount[];
	/** Keys: `poor` (1–2), `fair` (3), `good` (4–5). */
	mood_by_sleep: PatternGroup[];
	/** Keys: `night`, `morning`, `afternoon`, `evening`. */
	mood_by_time_of_day: PatternGroup[];
	/** Keys: `monday` … `sunday`. */
	mood_by_weekday: PatternGroup[];
	strategies: PatternStrategy[];
}

export interface PatternCount {
	key: string;
	label: string;
	count: number;
}

export interface PatternGroup {
	key: string;
	average: number;
	count: number;
}

export interface PatternStrategy {
	label: string;
	/** Positive: intensity went down by this much on average. */
	average_drop: number;
	count: number;
}

// ---- about ----

export interface AppInfo {
	version: string;
	/** Unix seconds when this binary was built. */
	built_at: number;
	releases_url: string;
}

// ---- crisis lines (static, available without unlocking) ----

export interface CrisisLine {
	/** As displayed, e.g. `800 70 2222`. */
	number: string;
	emergency: boolean;
	hours: string;
	free: boolean;
	/** By UI locale: `pl`, `en`, `uk`. */
	name: Record<string, string>;
	description: Record<string, string>;
}

export interface CrisisDirectory {
	country: string;
	/** `YYYY-MM-DD` */
	verified_at: string;
	sources: string[];
	lines: CrisisLine[];
}

export interface CrisisInfo {
	countries: string[];
	directory: CrisisDirectory | null;
}

// ---- backups ----

export interface BackupSummary {
	records: number;
	/** The backup comes from the vault that is currently unlocked. */
	same_vault: boolean;
}

export interface BackupInfo {
	auto_enabled: boolean;
	/** Newest automatic backup on this device (RFC 3339), if any. */
	auto_latest: string | null;
	auto_count: number;
	/** The last automatic backup that was due could not be written. */
	auto_failed: boolean;
	/** The system file manager can show the folder (desktop only). */
	can_reveal: boolean;
	last_export: string | null;
}

export type BackupSecret = 'password' | 'recovery_phrase';
export type ImportMode = 'merge' | 'replace';

export interface ImportReport {
	added: number;
	updated: number;
	unchanged: number;
	skipped: number;
}

export interface ImportOutcome {
	/** `true` when there was no vault and the backup became the vault. */
	restored: boolean;
	report: ImportReport;
}

// ---- report for a therapist (X-3) ----

/** Document model rendered to PDF by Rust; all strings are final, translated text. */
export interface ReportDoc {
	title: string;
	subtitle: string;
	summary: string[];
	sections: ReportSection[];
	footer: string;
	/** With `{page}` and `{pages}`. */
	page_label: string;
}

export interface ReportSection {
	heading: string;
	items: ReportItem[];
}

export interface ReportItem {
	title: string;
	meta: string;
	fields: ReportField[];
}

export interface ReportField {
	label: string;
	value: string;
}

/** Sync between the user's own devices through a folder Syncthing carries (D-22). */
/** Settings → Transcription (E-19, D-23): the model on this device. */
export interface TranscriptionStatus {
	/** The model is imported, so transcription is on. */
	installed: boolean;
	/** An import or a transcription is running. */
	busy: boolean;
	download_bytes: number;
	disk_bytes: number;
	/** Total memory of the device, where it can be read (Android, Linux). */
	total_memory: number | null;
	low_memory: boolean;
}

/** Progress of a model import (bytes) or a transcription (pieces). */
export interface TranscriptionProgress {
	done: number;
	total: number;
}

export interface SyncStatus {
	/** The folder, when sync is on. */
	location: string | null;
	devices: SyncDevice[];
	/** The folder holds a journal from another device, which this one can join. */
	other_journal: boolean;
	last_sync: Timestamp | null;
	problem: 'folder_unavailable' | 'damaged_file' | 'merge_failed' | 'write_failed' | null;
	/** What the last sync changed here; the UI reloads when it is not 0. */
	received: number;
}

export interface SyncDevice {
	this_device: boolean;
	/** When its file last changed. */
	changed_at: Timestamp;
	other_journal: boolean;
}

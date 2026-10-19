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

export type EntryKindTag = 'mood' | 'event' | 'fear' | 'sleep' | 'nightmare' | 'note';

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
	| 'note';

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
}

/** What the UI submits; the core sets id (for new entries), schema and timestamps. */
export interface EntryDraft {
	id?: Id;
	kind: EntryKind;
	occurred_at?: Timestamp;
	mood?: number | null;
	intensity?: number | null;
	tags?: Id[];
	note?: string;
	incomplete?: boolean;
	sensitive?: boolean;
}

export interface EntryFilter {
	kinds?: EntryKindTag[];
	tags?: Id[];
	from?: Timestamp;
	to?: Timestamp;
	text?: string;
	incomplete_only?: boolean;
}

// ---- tags, plan, settings ----

export interface Tag {
	id: Id;
	name: string;
	/** `#rrggbb` */
	color: string;
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
	locale: 'pl' | 'en' | 'uk';
	crisis_country: string;
	reminders: Reminder[];
}

// ---- patterns ----

export type StatsRange = 'week' | 'month' | 'three_months';

export interface MoodPoint {
	/** Calendar date, `YYYY-MM-DD`. */
	date: string;
	average: number;
	count: number;
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

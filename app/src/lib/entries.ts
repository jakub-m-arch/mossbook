// Pure helpers for entries: kind tags, empty kinds, drafts and previews.

import type { Emotion, Entry, EntryDraft, EntryKind, EntryKindTag, Symptom } from './api/index.js';

/** Types offered by quick entry, the way most entries are started. */
export const CREATABLE_KINDS: EntryKindTag[] = [
	'mood',
	'fear',
	'event',
	'sleep',
	'nightmare',
	'note'
];

/** Every type, e.g. for filters. A session (D-26) is started from "For the next session". */
export const ALL_KINDS: EntryKindTag[] = [...CREATABLE_KINDS, 'session'];

export const EMOTIONS = [
	'anxiety',
	'sadness',
	'anger',
	'shame',
	'guilt',
	'relief',
	'calm',
	'joy',
	'loneliness',
	'numbness',
	'overwhelm'
] as const;

export const SYMPTOMS = [
	'racing_heart',
	'shortness_of_breath',
	'chest_tightness',
	'sweating',
	'trembling',
	'nausea',
	'dizziness',
	'muscle_tension',
	'hot_or_cold_flushes',
	'tingling',
	'unreality'
] as const;

export function kindTag(kind: EntryKind): EntryKindTag {
	return typeof kind === 'string' ? kind : (Object.keys(kind)[0] as EntryKindTag);
}

export function emptyKind(tag: EntryKindTag): EntryKind {
	switch (tag) {
		case 'mood':
			return { mood: { energy: null, emotions: [] } };
		case 'event':
			return { event: { what: '', place: '', body: '', thoughts: '', helped: '' } };
		case 'fear':
			return {
				fear: { trigger: '', symptoms: [], avoidance: null, strategy: null, intensity_after: null }
			};
		case 'sleep':
			return {
				sleep: { bedtime: null, wake: null, quality: null, awakenings: null, nightmare: false }
			};
		case 'nightmare':
			return { nightmare: { content: '', rescripted: '', rehearsals: [] } };
		case 'note':
			return 'note';
		case 'session':
			return { session: { agreed: '', todo: [], discussed: [] } };
	}
}

/** Deep copy of JSON-shaped data. Unlike `structuredClone` it also accepts
 * Svelte `$state` proxies, which are not cloneable. */
export function jsonCopy<T>(value: T): T {
	return JSON.parse(JSON.stringify(value)) as T;
}

/** Editable copy of an entry. Saving a full form completes a quick entry. */
export function draftFromEntry(entry: Entry): EntryDraft {
	return {
		id: entry.id,
		kind: jsonCopy(entry.kind),
		occurred_at: entry.occurred_at,
		mood: entry.mood,
		intensity: entry.intensity,
		tags: [...entry.tags],
		note: entry.note,
		incomplete: false,
		sensitive: entry.sensitive,
		voice: entry.voice ? { ...entry.voice } : null,
		discuss: entry.discuss ?? false
	};
}

export function newDraft(tag: EntryKindTag, options: { discuss?: boolean } = {}): EntryDraft {
	// Content written in the form is hidden on the timeline until tapped, unless
	// the user unticks it (E-16, D-24). Quick entries, a single number, are not.
	return {
		kind: emptyKind(tag),
		mood: null,
		intensity: null,
		tags: [],
		note: '',
		sensitive: true,
		discuss: tag !== 'session' && (options.discuss ?? false)
	};
}

/** Free-text fields of an entry, in reading order. */
export function entryTexts(entry: Entry): string[] {
	const k = entry.kind;
	const kind: string[] =
		typeof k === 'string'
			? []
			: 'event' in k
				? [k.event.what, k.event.place, k.event.helped]
				: 'fear' in k
					? [k.fear.trigger, k.fear.strategy ?? '']
					: 'nightmare' in k
						? [k.nightmare.rescripted]
						: 'session' in k
							? [k.session.agreed, ...k.session.todo.map((item) => item.text)]
							: [];
	return [...kind, entry.note].map((s) => s.trim()).filter(Boolean);
}

/** A short single-line preview for the timeline. */
export function preview(entry: Entry, max = 120): string {
	const text = entryTexts(entry).join(' · ').replace(/\s+/g, ' ');
	return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** What a sleep entry says at a glance: minutes asleep (bedtime to wake), quality, awakenings. */
export function sleepFacts(entry: Entry): {
	minutes: number | null;
	quality: number | null;
	awakenings: number | null;
} | null {
	if (typeof entry.kind === 'string' || !('sleep' in entry.kind)) return null;
	const { bedtime, wake, quality, awakenings } = entry.kind.sleep;
	const span = bedtime && wake ? (Date.parse(wake) - Date.parse(bedtime)) / 60_000 : NaN;
	return {
		// Only a plausible night counts; anything else was mistyped.
		minutes: span > 0 && span < 24 * 60 ? Math.round(span) : null,
		quality,
		awakenings
	};
}

/** Translation key or custom text for an emotion or symptom. */
export function chipKey(item: Emotion | Symptom): { key: string } | { custom: string } {
	return typeof item === 'string' ? { key: item } : { custom: item.custom };
}

export interface DayGroup<T> {
	/** Local `YYYY-MM-DD`. */
	key: string;
	/** `today`, `yesterday`, or `null` for older days (format `key` instead). */
	relative: 'today' | 'yesterday' | null;
	items: T[];
}

const localDay = (d: Date) =>
	`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/** Groups entries (already sorted newest first) by local calendar day. */
export function groupByDay<T extends { occurred_at: string }>(
	entries: T[],
	now = new Date()
): DayGroup<T>[] {
	const today = localDay(now);
	const yesterdayDate = new Date(now);
	yesterdayDate.setDate(now.getDate() - 1);
	const yesterday = localDay(yesterdayDate);
	const groups: DayGroup<T>[] = [];
	for (const entry of entries) {
		const key = localDay(new Date(entry.occurred_at));
		let group = groups.at(-1);
		if (!group || group.key !== key) {
			group = {
				key,
				relative: key === today ? 'today' : key === yesterday ? 'yesterday' : null,
				items: []
			};
			groups.push(group);
		}
		group.items.push(entry);
	}
	return groups;
}

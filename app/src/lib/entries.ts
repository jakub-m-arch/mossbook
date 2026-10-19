// Pure helpers for entries: kind tags, empty kinds, drafts and previews.

import type { Emotion, Entry, EntryDraft, EntryKind, EntryKindTag, Symptom } from './api/index.js';

/** Types offered when creating entries. Nightmare (IRT) arrives in v1 (E-5). */
export const CREATABLE_KINDS: EntryKindTag[] = ['mood', 'fear', 'event', 'sleep', 'note'];

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
		sensitive: entry.sensitive
	};
}

export function newDraft(tag: EntryKindTag): EntryDraft {
	return { kind: emptyKind(tag), mood: null, intensity: null, tags: [], note: '' };
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
						: [];
	return [...kind, entry.note].map((s) => s.trim()).filter(Boolean);
}

/** A short single-line preview for the timeline. */
export function preview(entry: Entry, max = 120): string {
	const text = entryTexts(entry).join(' · ').replace(/\s+/g, ' ');
	return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** Translation key or custom text for an emotion or symptom. */
export function chipKey(item: Emotion | Symptom): { key: string } | { custom: string } {
	return typeof item === 'string' ? { key: item } : { custom: item.custom };
}

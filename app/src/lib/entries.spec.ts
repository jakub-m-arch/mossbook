import { describe, expect, it } from 'vitest';

import type { Entry } from './api/index.js';
import {
	draftFromEntry,
	emptyKind,
	groupByDay,
	kindTag,
	newDraft,
	preview,
	sleepFacts
} from './entries';

function entry(overrides: Partial<Entry> = {}): Entry {
	return {
		id: 'id-1',
		schema: 1,
		created_at: '2026-10-19T20:00:00+02:00',
		updated_at: '2026-10-19T20:00:00+02:00',
		occurred_at: '2026-10-19T20:00:00+02:00',
		kind: emptyKind('fear'),
		mood: null,
		intensity: 7,
		tags: ['t1'],
		note: '',
		incomplete: true,
		sensitive: false,
		...overrides
	};
}

describe('entries helpers', () => {
	it('maps kinds to tags and back', () => {
		for (const tag of ['mood', 'event', 'fear', 'sleep', 'nightmare', 'note', 'session'] as const) {
			expect(kindTag(emptyKind(tag))).toBe(tag);
		}
	});

	it('drafts copy an entry and complete quick entries', () => {
		const e = entry();
		const draft = draftFromEntry(e);
		expect(draft).toMatchObject({ id: 'id-1', intensity: 7, incomplete: false });
		draft.tags!.push('t2');
		expect(e.tags).toEqual(['t1']);
		expect(draft.voice).toBeNull();
		// The recording stays attached when an entry is edited (E-18).
		const voice = { id: 'v1', duration_ms: 2000, mime: 'audio/ogg; codecs=opus' };
		expect(draftFromEntry({ ...entry(), voice }).voice).toEqual(voice);
		// Entries held in Svelte state are proxies, which structuredClone rejects.
		expect(draftFromEntry(new Proxy(entry(), {})).kind).toEqual(e.kind);
		expect(newDraft('note')).toEqual({
			kind: 'note',
			mood: null,
			intensity: null,
			tags: [],
			note: '',
			// Hidden on the timeline unless unticked (D-24).
			sensitive: true,
			discuss: false
		});
		expect(newDraft('nightmare').sensitive).toBe(true);
		// "Add a question" (D-26); a session is never marked for one itself.
		expect(newDraft('note', { discuss: true }).discuss).toBe(true);
		expect(newDraft('session', { discuss: true }).discuss).toBe(false);
		expect(draftFromEntry(entry({ discuss: true })).discuss).toBe(true);
		expect(draftFromEntry(entry()).discuss).toBe(false);
	});

	it('previews join free text and truncate', () => {
		const e = entry({
			kind: {
				fear: {
					trigger: 'crowded  tram',
					symptoms: [],
					avoidance: null,
					strategy: 'breathing',
					intensity_after: 3
				}
			},
			note: 'x'.repeat(200)
		});
		const p = preview(e, 40);
		expect(p.startsWith('crowded tram · breathing · xxx')).toBe(true);
		expect([...p]).toHaveLength(40);
		expect(preview(entry({ kind: 'note', note: '' }))).toBe('');
	});
});

describe('groupByDay', () => {
	it('groups by local day with relative labels', () => {
		const now = new Date(2026, 9, 19, 21, 0);
		const at = (d: number, h: number) => ({ occurred_at: new Date(2026, 9, d, h).toISOString() });
		const groups = groupByDay([at(19, 20), at(19, 8), at(18, 23), at(15, 12)], now);
		expect(groups.map((g) => [g.key, g.relative, g.items.length])).toEqual([
			['2026-10-19', 'today', 2],
			['2026-10-18', 'yesterday', 1],
			['2026-10-15', null, 1]
		]);
		expect(groupByDay([], now)).toEqual([]);
	});

	it('sums up a night of sleep', () => {
		const night = (bedtime: string | null, wake: string | null) =>
			entry({
				kind: {
					sleep: { bedtime, wake, quality: 4, awakenings: 2, nightmare: false }
				}
			});
		expect(sleepFacts(night('2026-10-18T23:10:00+02:00', '2026-10-19T06:50:00+02:00'))).toEqual({
			minutes: 460,
			quality: 4,
			awakenings: 2
		});
		// Missing or implausible times give no length, the rest stays.
		expect(sleepFacts(night(null, '2026-10-19T06:50:00+02:00'))?.minutes).toBeNull();
		expect(
			sleepFacts(night('2026-10-19T07:00:00+02:00', '2026-10-19T06:00:00+02:00'))?.minutes
		).toBeNull();
		expect(sleepFacts(entry())).toBeNull();
	});
});

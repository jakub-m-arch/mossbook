import { describe, expect, it } from 'vitest';

import type { Entry } from './api/index.js';
import { draftFromEntry, emptyKind, groupByDay, kindTag, newDraft, preview } from './entries';

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
		for (const tag of ['mood', 'event', 'fear', 'sleep', 'nightmare', 'note'] as const) {
			expect(kindTag(emptyKind(tag))).toBe(tag);
		}
	});

	it('drafts copy an entry and complete quick entries', () => {
		const e = entry();
		const draft = draftFromEntry(e);
		expect(draft).toMatchObject({ id: 'id-1', intensity: 7, incomplete: false });
		draft.tags!.push('t2');
		expect(e.tags).toEqual(['t1']);
		// Entries held in Svelte state are proxies, which structuredClone rejects.
		expect(draftFromEntry(new Proxy(entry(), {})).kind).toEqual(e.kind);
		expect(newDraft('note')).toEqual({
			kind: 'note',
			mood: null,
			intensity: null,
			tags: [],
			note: ''
		});
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
});

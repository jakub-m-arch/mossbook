// Times in the report are shown in the device's zone; pin it so CI (UTC) matches.
process.env.TZ = 'Europe/Warsaw';

import { describe, expect, it } from 'vitest';

import type { Entry, Tag } from './api/index.js';
import { emptyKind } from './entries';
import { setLocale, t } from './i18n/index.svelte';
import { buildReport } from './report';

function entry(at: string, overrides: Partial<Entry>): Entry {
	return {
		id: at,
		schema: 1,
		created_at: at,
		updated_at: at,
		occurred_at: at,
		kind: 'note',
		mood: null,
		intensity: null,
		tags: [],
		note: '',
		incomplete: false,
		sensitive: false,
		...overrides
	};
}

describe('buildReport', () => {
	setLocale('pl');
	const tags = new Map<string, Tag>([['t1', { id: 't1', name: 'Praca', color: '#000000' }]]);
	const fear = entry('2026-10-19T21:30:00+02:00', {
		kind: {
			fear: {
				trigger: 'tłum w tramwaju',
				symptoms: ['racing_heart', { custom: 'zimne dłonie' }],
				avoidance: true,
				strategy: '  ',
				intensity_after: 3
			}
		},
		intensity: 8,
		tags: ['t1', 'gone'],
		note: 'potem lepiej'
	});
	const mood = entry('2026-10-18T09:00:00+02:00', { kind: emptyKind('mood'), mood: 4 });
	const options = {
		from: '2026-10-01',
		to: '2026-10-19',
		tags,
		t,
		locale: 'pl'
	};
	const doc = buildReport([fear, mood], options);

	it('orders days and entries oldest first, with a summary', () => {
		expect(doc.title).toBe('Mossbook — moje zapiski');
		expect(doc.subtitle).toBe('1 października 2026 – 19 października 2026');
		expect(doc.summary).toEqual(['Liczba wpisów: 2', 'Średnia ocena samopoczucia: 4']);
		expect(doc.sections.map((s) => s.items.length)).toEqual([1, 1]);
		expect(doc.sections[0].heading).toContain('18 października');
		expect(doc.page_label).toBe('Strona {page} z {pages}');
	});

	it('lists filled fields only, translated, with meta on one line', () => {
		const item = doc.sections[1].items[0];
		expect(item.title).toBe('21:30 · Lęk / wyzwalacz');
		expect(item.meta).toBe(
			'Intensywność: 8/10 · Było unikanie · Intensywność po: 3/10 · Tagi: Praca'
		);
		expect(item.fields).toEqual([
			{ label: 'Co to wywołało', value: 'tłum w tramwaju' },
			{ label: 'Objawy w ciele', value: 'kołatanie serca, zimne dłonie' },
			{ label: 'Notatka', value: 'potem lepiej' }
		]);
		expect(doc.sections[0].items[0].meta).toBe('Samopoczucie: dobrze');
	});

	it('marks the footer when the PDF will be encrypted', () => {
		const encrypted = buildReport([fear, mood], { ...options, encrypted: true });
		expect(encrypted.footer).toBe('Mossbook · ten plik jest zabezpieczony hasłem');
	});
});

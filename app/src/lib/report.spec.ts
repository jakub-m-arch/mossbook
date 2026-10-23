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
			{ label: 'Odczucia w ciele', value: 'kołatanie serca, zimne dłonie' },
			{ label: 'Notatka', value: 'potem lepiej' }
		]);
		expect(doc.sections[0].items[0].meta).toBe('Samopoczucie: dobrze');
	});

	it('marks the footer when the PDF will be encrypted', () => {
		const encrypted = buildReport([fear, mood], { ...options, encrypted: true });
		expect(encrypted.footer).toBe('Mossbook · ten plik jest zabezpieczony hasłem');
	});

	it('mentions a voice note and its length, without the recording', () => {
		const spoken = entry('2026-10-19T08:00:00+02:00', {
			voice: { id: 'v1', duration_ms: 151_000, mime: 'audio/ogg; codecs=opus' }
		});
		const item = buildReport([spoken], options).sections[0].items[0];
		expect(item.meta).toBe('Notatka głosowa, 2:31');
		expect(item.fields).toEqual([]);
	});

	it('shows a session and can start with the topics, whatever the period', () => {
		const topic = entry('2026-01-05T10:00:00+01:00', { note: 'stary temat', discuss: true });
		const session = entry('2026-10-19T18:00:00+02:00', {
			kind: {
				session: {
					agreed: 'krótsze wieczory',
					todo: [
						{ text: 'spacer ', done: true },
						{ text: 'zapisywać wieczory', done: false }
					],
					discussed: ['a', 'b']
				}
			}
		});
		const doc = buildReport([session], { ...options, topics: [topic] });
		expect(doc.sections.map((s) => s.heading)).toEqual([
			'Tematy na tę sesję',
			'poniedziałek, 19 października 2026'
		]);
		expect(doc.sections[0].items[0].title).toMatch(/2026, 10:00 · Notatka$/);
		expect(doc.sections[0].items[0].fields).toEqual([{ label: 'Notatka', value: 'stary temat' }]);
		const s = doc.sections[1].items[0];
		expect(s.title).toBe('18:00 · Sesja');
		expect(s.meta).toBe('Omówione wpisy: 2');
		expect(s.fields).toEqual([
			{ label: 'Co ustaliliśmy', value: 'krótsze wieczory' },
			{
				label: 'Co mam zrobić do następnej sesji',
				value: '• spacer (zrobione)\n• zapisywać wieczory'
			}
		]);
		// Not counted with the period's entries.
		expect(doc.summary[0]).toBe('Liczba wpisów: 1');
		expect(buildReport([session], options).sections).toHaveLength(1);
	});

	it('includes the text of a voice note only when asked to', () => {
		const spoken = entry('2026-10-19T08:00:00+02:00', {
			note: 'krótko',
			voice: {
				id: 'v1',
				duration_ms: 151_000,
				mime: 'audio/ogg; codecs=opus',
				transcript: ' Synthetic transcript. '
			}
		});
		const without = buildReport([spoken], options).sections[0].items[0];
		expect(without.fields).toEqual([{ label: 'Notatka', value: 'krótko' }]);

		const withText = buildReport([spoken], { ...options, transcripts: true }).sections[0].items[0];
		expect(withText.meta).toBe('Notatka głosowa, 2:31');
		expect(withText.fields).toEqual([
			{ label: 'Notatka', value: 'krótko' },
			{ label: 'Tekst nagrania', value: 'Synthetic transcript.' }
		]);

		const silent = entry('2026-10-19T09:00:00+02:00', {
			voice: { id: 'v2', duration_ms: 1_000, mime: 'audio/ogg; codecs=opus', transcript: '  ' }
		});
		expect(
			buildReport([silent], { ...options, transcripts: true }).sections[0].items[0].fields
		).toEqual([]);
	});
});

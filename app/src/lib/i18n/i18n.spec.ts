import { describe, expect, it } from 'vitest';

import en from './en.json';
import { detectLocale, setLocale, t } from './index.svelte';
import pl from './pl.json';
import uk from './uk.json';

type Dictionary = { [key: string]: string | Dictionary };

function keys(dict: Dictionary, prefix = ''): string[] {
	return Object.entries(dict).flatMap(([k, v]) =>
		typeof v === 'string' ? [prefix + k] : keys(v, `${prefix}${k}.`)
	);
}

function placeholders(dict: Dictionary): Record<string, string[]> {
	return Object.fromEntries(
		keys(dict).map((k) => {
			const text = k.split('.').reduce<string | Dictionary>((d, p) => (d as Dictionary)[p], dict);
			return [k, [...(text as string).matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort()];
		})
	);
}

describe('i18n', () => {
	it('all languages have the same keys and placeholders', () => {
		expect(keys(pl).sort()).toEqual(keys(en).sort());
		expect(keys(uk).sort()).toEqual(keys(en).sort());
		expect(placeholders(pl)).toEqual(placeholders(en));
		expect(placeholders(uk)).toEqual(placeholders(en));
	});

	it('detects the system language with English fallback', () => {
		expect(detectLocale(['pl-PL', 'en-US'])).toBe('pl');
		expect(detectLocale(['de-DE', 'uk'])).toBe('uk');
		expect(detectLocale(['de-DE'])).toBe('en');
		expect(detectLocale([])).toBe('en');
	});

	it('translates with placeholders and falls back to the key', () => {
		setLocale('pl');
		expect(t('lock.wait', { seconds: 4 })).toBe('Następna próba za 4 s');
		setLocale('en');
		expect(t('lock.wait', { seconds: 4 })).toBe('Next attempt in 4 s');
		expect(t('no.such.key')).toBe('no.such.key');
	});

	it('texts keep a calm tone: no exclamation marks (SPEC §8.5)', () => {
		for (const dict of [pl, en, uk]) {
			for (const key of keys(dict)) {
				const text = key
					.split('.')
					.reduce<string | Dictionary>((d, p) => (d as Dictionary)[p], dict);
				expect(text, key).not.toContain('!');
			}
		}
	});
});

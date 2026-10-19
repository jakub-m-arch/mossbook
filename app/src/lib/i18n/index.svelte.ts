// Minimal i18n: nested JSON dictionaries and `{name}` placeholders.
// Before unlock the language comes from the system (decision D-2); after
// unlock the saved setting wins.

import en from './en.json';
import pl from './pl.json';
import uk from './uk.json';

export const LOCALES = ['pl', 'en', 'uk'] as const;
export type Locale = (typeof LOCALES)[number];

type Dictionary = { [key: string]: string | Dictionary };
const dictionaries: Record<Locale, Dictionary> = { pl, en, uk };
const FALLBACK: Locale = 'en';

/** First supported language from the system preferences, else English. */
export function detectLocale(
	languages: readonly string[] = globalThis.navigator?.languages ?? []
): Locale {
	for (const tag of languages) {
		const base = tag.toLowerCase().split('-')[0];
		if ((LOCALES as readonly string[]).includes(base)) return base as Locale;
	}
	return FALLBACK;
}

let current = $state<Locale>(detectLocale());

export function locale(): Locale {
	return current;
}

export function setLocale(next: Locale): void {
	current = next;
	if (typeof document !== 'undefined') document.documentElement.lang = next;
}

function lookup(dict: Dictionary, key: string): string | undefined {
	let node: string | Dictionary | undefined = dict;
	for (const part of key.split('.')) {
		if (typeof node !== 'object') return undefined;
		node = node[part];
	}
	return typeof node === 'string' ? node : undefined;
}

/** Translates `key` in the current language, falling back to English, then to the key. */
export function t(key: string, params: Record<string, string | number> = {}): string {
	const text = lookup(dictionaries[current], key) ?? lookup(dictionaries[FALLBACK], key) ?? key;
	return text.replace(/\{(\w+)\}/g, (match, name: string) =>
		name in params ? String(params[name]) : match
	);
}

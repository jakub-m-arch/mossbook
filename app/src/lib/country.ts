// Country for crisis numbers before unlock: taken from the system language
// region (decision D-2), since settings are encrypted.

const LANGUAGE_COUNTRY: Record<string, string> = { pl: 'PL', uk: 'UA' };

/** `pl-PL` → `PL`; a bare `pl` maps to its main country; unknown → `null`. */
export function detectCountry(
	languages: readonly string[] = globalThis.navigator?.languages ?? []
): string | null {
	for (const tag of languages) {
		const [language, region] = tag.split('-');
		if (region && /^[A-Za-z]{2}$/.test(region)) return region.toUpperCase();
		const mapped = LANGUAGE_COUNTRY[language.toLowerCase()];
		if (mapped) return mapped;
	}
	return null;
}

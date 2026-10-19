import { describe, expect, it } from 'vitest';

import { detectCountry } from './country';

describe('detectCountry', () => {
	it('uses the region of the first tag that has one', () => {
		expect(detectCountry(['pl-PL', 'en-US'])).toBe('PL');
		expect(detectCountry(['en-GB'])).toBe('GB');
		expect(detectCountry(['uk'])).toBe('UA');
		expect(detectCountry(['pl'])).toBe('PL');
		expect(detectCountry(['de', 'en'])).toBeNull();
		expect(detectCountry(['zh-Hant-TW'])).toBeNull();
		expect(detectCountry([])).toBeNull();
	});
});

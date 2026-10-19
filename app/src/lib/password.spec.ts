import { describe, expect, it } from 'vitest';

import { passwordStrength } from './password';

describe('passwordStrength', () => {
	it('rejects short passwords by character count', () => {
		expect(passwordStrength('')).toBe(0);
		expect(passwordStrength('1234567')).toBe(0);
		expect(passwordStrength('zażółć')).toBe(0);
	});

	it('flags common, repetitive and sequential passwords as weak', () => {
		for (const pw of [
			'password1',
			'Haslo2026',
			'aaaaaaaa',
			'abababab',
			'12345678',
			'abcdefgh',
			'87654321'
		]) {
			expect(passwordStrength(pw), pw).toBe(1);
		}
	});

	it('rates length, variety and passphrases', () => {
		expect(passwordStrength('kotek-mruczy')).toBe(2);
		expect(passwordStrength('Kot7Mruczy!x')).toBe(3);
		expect(passwordStrength('zielony mech na kamieniu')).toBe(3);
		expect(passwordStrength('trzy krotkie slowa')).toBe(3);
		expect(passwordStrength('kotmruczy')).toBe(1);
	});
});

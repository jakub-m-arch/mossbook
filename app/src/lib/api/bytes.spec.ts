import { describe, expect, it } from 'vitest';

import { base64ToBytes, bytesToBase64 } from './bytes';

describe('base64 over IPC', () => {
	it('round-trips every byte value and large inputs', () => {
		const all = Uint8Array.from({ length: 256 }, (_, i) => i);
		expect(base64ToBytes(bytesToBase64(all))).toEqual(all);
		const big = Uint8Array.from({ length: 200_000 }, (_, i) => (i * 31) % 256);
		expect(base64ToBytes(bytesToBase64(big))).toEqual(big);
		expect(bytesToBase64(new Uint8Array())).toBe('');
	});

	it('matches the standard alphabet with padding', () => {
		expect(bytesToBase64(new TextEncoder().encode('OggS'))).toBe('T2dnUw==');
	});
});

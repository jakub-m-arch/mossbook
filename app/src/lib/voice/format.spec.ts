import { describe, expect, it } from 'vitest';

import { formatClock } from './format';

describe('formatClock', () => {
	it('shows minutes and two-digit seconds', () => {
		expect(formatClock(0)).toBe('0:00');
		expect(formatClock(999)).toBe('0:00');
		expect(formatClock(61_500)).toBe('1:01');
		expect(formatClock(300_000)).toBe('5:00');
		expect(formatClock(-5)).toBe('0:00');
	});
});

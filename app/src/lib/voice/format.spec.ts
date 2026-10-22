import { describe, expect, it } from 'vitest';

import { formatBytes, formatClock } from './format';

describe('formatClock', () => {
	it('shows minutes and two-digit seconds', () => {
		expect(formatClock(0)).toBe('0:00');
		expect(formatClock(999)).toBe('0:00');
		expect(formatClock(61_500)).toBe('1:01');
		expect(formatClock(300_000)).toBe('5:00');
		expect(formatClock(-5)).toBe('0:00');
	});
});

describe('formatBytes', () => {
	it('uses megabytes below a gigabyte and one decimal above', () => {
		expect(formatBytes(487_170_055, 'en')).toBe('487 MB');
		expect(formatBytes(670_478_772, 'pl')).toBe('670 MB');
		expect(formatBytes(3_900_000_000, 'pl')).toBe('3,9 GB');
		expect(formatBytes(8e9, 'en')).toBe('8 GB');
	});
});

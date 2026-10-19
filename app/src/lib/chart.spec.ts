import { describe, expect, it } from 'vitest';

import { addDays, dayIndex, localToday, nearest, rangeDays, scale, xTicks } from './chart';

describe('chart geometry', () => {
	it('counts calendar days across DST and months', () => {
		expect(dayIndex('2026-10-24', '2026-10-26')).toBe(2); // DST change in Europe
		expect(addDays('2026-10-30', 3)).toBe('2026-11-02');
		expect(addDays('2026-03-01', -1)).toBe('2026-02-28');
		expect(localToday(new Date(2026, 9, 19, 23, 59))).toBe('2026-10-19');
	});

	it('builds ranges and ticks ending today', () => {
		expect(rangeDays('week', '2026-10-19')).toEqual(['2026-10-12', '2026-10-19']);
		const ticks = xTicks('2026-07-21', '2026-10-19');
		expect(ticks.at(-1)).toBe('2026-10-19');
		expect(ticks.length).toBeLessThanOrEqual(6);
		expect(xTicks('2026-10-12', '2026-10-19', 8)).toHaveLength(8);
	});

	it('scales and finds the nearest point', () => {
		const y = scale([1, 5], [200, 0]);
		expect(y(1)).toBe(200);
		expect(y(5)).toBe(0);
		expect(y(3)).toBe(100);
		expect(scale([2, 2], [0, 10])(2)).toBe(5);
		expect(nearest([0, 10, 20], 14)).toBe(1);
		expect(nearest([0, 10, 20], 16)).toBe(2);
	});
});

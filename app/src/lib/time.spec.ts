import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { endOfDay, fromLocalInput, startOfDay, toLocalInput, toRfc3339 } from './time';

describe('time conversions', () => {
	const tz = process.env.TZ;
	beforeAll(() => {
		process.env.TZ = 'Europe/Warsaw';
	});
	afterAll(() => {
		process.env.TZ = tz;
	});

	it('round-trips local input values with the local offset', () => {
		expect(fromLocalInput('2026-10-19T21:30')).toBe('2026-10-19T21:30:00+02:00');
		expect(fromLocalInput('2026-12-01T08:05')).toBe('2026-12-01T08:05:00+01:00');
		expect(toLocalInput('2026-10-19T21:30:00+02:00')).toBe('2026-10-19T21:30');
		// A timestamp recorded elsewhere is shown in the device's zone.
		expect(toLocalInput('2026-10-19T12:00:00-04:00')).toBe('2026-10-19T18:00');
	});

	it('formats offsets and day bounds', () => {
		expect(toRfc3339(new Date('2026-07-01T00:00:00Z'))).toBe('2026-07-01T02:00:00+02:00');
		expect(startOfDay('2026-10-25')).toBe('2026-10-25T00:00:00+02:00');
		// Day of the DST change: next day starts in winter time.
		expect(endOfDay('2026-10-25')).toBe('2026-10-26T00:00:00+01:00');
	});
});

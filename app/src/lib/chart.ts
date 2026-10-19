// Pure geometry for the mood chart (A-1). Dates are calendar days (`YYYY-MM-DD`)
// handled in UTC so DST changes never shift a day.

import type { StatsRange } from './api/index.js';

export const RANGE_DAYS: Record<StatsRange, number> = { week: 7, month: 30, three_months: 90 };

const DAY_MS = 86_400_000;

const toUtc = (day: string) => {
	const [y, m, d] = day.split('-').map(Number);
	return Date.UTC(y, m - 1, d);
};

const fromUtc = (ms: number) => new Date(ms).toISOString().slice(0, 10);

/** Whole days from `from` to `day`. */
export function dayIndex(from: string, day: string): number {
	return Math.round((toUtc(day) - toUtc(from)) / DAY_MS);
}

export function addDays(day: string, n: number): string {
	return fromUtc(toUtc(day) + n * DAY_MS);
}

/** Today's local calendar day. */
export function localToday(now = new Date()): string {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/** `[from, to]` days shown for a range ending today (matches Rust's `StatsRange::bounds`). */
export function rangeDays(range: StatsRange, today: string): [string, string] {
	return [addDays(today, -RANGE_DAYS[range]), today];
}

/** At most `max` evenly spaced tick days, always ending on `to`. */
export function xTicks(from: string, to: string, max = 6): string[] {
	const span = dayIndex(from, to);
	const step = Math.max(1, Math.ceil(span / (max - 1)));
	const ticks: string[] = [];
	for (let i = span; i >= 0; i -= step) ticks.unshift(addDays(from, i));
	return ticks;
}

/** Linear scale. */
export function scale(domain: [number, number], range: [number, number]) {
	const [d0, d1] = domain;
	const [r0, r1] = range;
	return (v: number) => (d1 === d0 ? (r0 + r1) / 2 : r0 + ((v - d0) / (d1 - d0)) * (r1 - r0));
}

/** Index of the point whose x is closest to `x` (points sorted by x). */
export function nearest(xs: number[], x: number): number {
	let best = 0;
	for (let i = 1; i < xs.length; i++) if (Math.abs(xs[i] - x) < Math.abs(xs[best] - x)) best = i;
	return best;
}

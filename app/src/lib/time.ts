// Conversions between RFC 3339 timestamps (with offset, as stored in Rust) and
// the local-time strings used by <input type="datetime-local">.

const pad = (n: number) => String(n).padStart(2, '0');

/** `2026-10-19T21:30:00+02:00` → `2026-10-19T21:30` in the device's time zone. */
export function toLocalInput(timestamp: string): string {
	const d = new Date(timestamp);
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** `2026-10-19T21:30` (local) → RFC 3339 with the device's offset at that moment. */
export function fromLocalInput(value: string): string {
	const d = new Date(value);
	return toRfc3339(d);
}

/** Date → RFC 3339 with the device's offset at that moment, e.g. `+02:00`. */
export function toRfc3339(d: Date): string {
	const offset = -d.getTimezoneOffset();
	const sign = offset >= 0 ? '+' : '-';
	const abs = Math.abs(offset);
	return (
		`${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` +
		`T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}` +
		`${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`
	);
}

/** Local `YYYY-MM-DD` (from <input type="date">) → start of that day, RFC 3339. */
export function startOfDay(date: string): string {
	return toRfc3339(new Date(`${date}T00:00`));
}

/** Local `YYYY-MM-DD` → start of the next day (exclusive upper bound). */
export function endOfDay(date: string): string {
	const d = new Date(`${date}T00:00`);
	d.setDate(d.getDate() + 1);
	return toRfc3339(d);
}

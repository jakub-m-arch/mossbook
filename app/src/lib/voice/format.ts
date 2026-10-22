/** A length in milliseconds as m:ss (voice notes are at most 5 minutes). */
export function formatClock(ms: number): string {
	const seconds = Math.max(0, Math.floor(ms / 1000));
	return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

/** A size for people: whole megabytes below a gigabyte, one decimal above (decimal units). */
export function formatBytes(bytes: number, locale: string): string {
	const gb = bytes / 1e9;
	if (gb >= 1) {
		return new Intl.NumberFormat(locale, {
			style: 'unit',
			unit: 'gigabyte',
			maximumFractionDigits: 1
		}).format(gb);
	}
	return new Intl.NumberFormat(locale, {
		style: 'unit',
		unit: 'megabyte',
		maximumFractionDigits: 0
	}).format(bytes / 1e6);
}

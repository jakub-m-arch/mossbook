<script lang="ts">
	// E-14: a month where each day is tinted by its average mood. One hue,
	// lighter to darker (sequential); no red. Tap a day to see its entries.
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';

	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { entriesList } from '#lib/api/index.js';
	import { addDays, localToday } from '#lib/chart.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { handleLocked } from '#lib/session.svelte.js';
	import { endOfDay, startOfDay } from '#lib/time.js';

	/** Tint strength per rounded mood level (1..5), as % of the series color. */
	const TINT = [0, 14, 28, 44, 60, 78];

	const today = localToday();
	let month = $state(today.slice(0, 7)); // YYYY-MM
	let days = $state<Record<string, { sum: number; count: number }>>({});

	let first = $derived(`${month}-01`);
	let length = $derived(new Date(Date.UTC(+month.slice(0, 4), +month.slice(5, 7), 0)).getUTCDate());
	let last = $derived(addDays(first, length - 1));
	/** Monday-first offset of the 1st. */
	let offset = $derived((new Date(`${first}T12:00`).getDay() + 6) % 7);
	let cells = $derived(Array.from({ length }, (_, i) => addDays(first, i)));

	let title = $derived(
		new Intl.DateTimeFormat(locale(), { month: 'long', year: 'numeric' }).format(
			new Date(`${first}T12:00`)
		)
	);
	let weekdays = $derived(
		// 2024-01-01 was a Monday.
		Array.from({ length: 7 }, (_, i) =>
			new Intl.DateTimeFormat(locale(), { weekday: 'narrow' }).format(new Date(2024, 0, 1 + i))
		)
	);
	let dayFormat = $derived(new Intl.DateTimeFormat(locale(), { day: 'numeric', month: 'long' }));
	let numberFormat = $derived(new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }));

	$effect(() => {
		const [from, to] = [first, last];
		entriesList({ from: startOfDay(from), to: endOfDay(to) })
			.then((entries) => {
				const next: Record<string, { sum: number; count: number }> = {};
				for (const e of entries) {
					if (e.mood === null) continue;
					const d = new Date(e.occurred_at);
					const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
					const day = next[key] ?? { sum: 0, count: 0 };
					next[key] = { sum: day.sum + e.mood, count: day.count + 1 };
				}
				days = next;
			})
			.catch(handleLocked);
	});

	function shift(by: number) {
		const d = new Date(Date.UTC(+month.slice(0, 4), +month.slice(5, 7) - 1 + by, 1));
		month = d.toISOString().slice(0, 7);
	}

	function label(day: string) {
		const value = days[day];
		const date = dayFormat.format(new Date(`${day}T12:00`));
		return value
			? t('calendar.day', { date, average: numberFormat.format(value.sum / value.count) })
			: t('calendar.dayEmpty', { date });
	}

	function tint(day: string) {
		const value = days[day];
		return value ? TINT[Math.round(value.sum / value.count)] : 0;
	}
</script>

<div class="calendar stack">
	<div class="header">
		<button
			type="button"
			class="icon"
			onclick={() => shift(-1)}
			aria-label={t('calendar.previous')}
		>
			<ChevronLeft size={22} />
		</button>
		<p class="month">{title}</p>
		<button
			type="button"
			class="icon"
			onclick={() => shift(1)}
			aria-label={t('calendar.next')}
			disabled={month >= today.slice(0, 7)}
		>
			<ChevronRight size={22} />
		</button>
	</div>

	<div class="grid" role="grid" aria-label={title}>
		{#each weekdays as wd, i (i)}<span class="weekday" aria-hidden="true">{wd}</span>{/each}
		{#each Array(offset).keys() as i (`pad-${i}`)}<span aria-hidden="true"></span>{/each}
		{#each cells as day (day)}
			<button
				type="button"
				class="day"
				class:today={day === today}
				class:future={day > today}
				disabled={day > today}
				style:--tint="{tint(day)}%"
				aria-label={label(day)}
				title={label(day)}
				onclick={() => goto(resolve(`/timeline?day=${day}`))}
			>
				{+day.slice(8)}
			</button>
		{/each}
	</div>

	<div class="legend" aria-hidden="true">
		<span>{t('calendar.legendLow')}</span>
		{#each TINT.slice(1) as level (level)}<span class="swatch" style:--tint="{level}%"
			></span>{/each}
		<span>{t('calendar.legendHigh')}</span>
	</div>
</div>

<style>
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.month {
		margin: 0;
		font-weight: 650;
		text-transform: capitalize;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(7, 1fr);
		gap: 4px;
	}
	.weekday {
		text-align: center;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--text-muted);
	}
	.day {
		aspect-ratio: 1;
		min-height: 0;
		min-width: 0;
		padding: 0;
		font-size: 0.85rem;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		color: var(--text);
		background: color-mix(in srgb, var(--chart-series) var(--tint), var(--surface-2));
		border-radius: var(--radius-sm);
	}
	.day:hover:not(:disabled) {
		background: color-mix(in srgb, var(--chart-series) var(--tint), var(--surface-3));
		outline: 2px solid var(--chart-series);
		outline-offset: -2px;
	}
	.day.today {
		box-shadow: inset 0 0 0 2px var(--text);
	}
	.day.future {
		opacity: 0.35;
	}
	.legend {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 4px;
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.swatch {
		width: 1.25rem;
		height: 0.75rem;
		border-radius: 3px;
		background: color-mix(in srgb, var(--chart-series) var(--tint), var(--surface-2));
	}
</style>

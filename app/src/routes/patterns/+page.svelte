<script lang="ts">
	// Patterns (A-1): observations only, never clinical conclusions (SPEC §4.4).
	import { statsMood } from '#lib/api/index.js';
	import type { MoodPoint, StatsRange } from '#lib/api/index.js';
	import { localToday, rangeDays } from '#lib/chart.js';
	import MoodChart from '#lib/components/MoodChart.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	const RANGES: Array<{ value: StatsRange; label: string }> = [
		{ value: 'week', label: 'patterns.week' },
		{ value: 'month', label: 'patterns.month' },
		{ value: 'three_months', label: 'patterns.threeMonths' }
	];

	let range = $state<StatsRange>('month');
	let points = $state<MoodPoint[] | null>(null);
	let error = $state('');

	let bounds = $derived(rangeDays(range, localToday()));
	let rated = $derived(points?.reduce((n, p) => n + p.count, 0) ?? 0);
	let average = $derived(
		points && rated > 0 ? points.reduce((sum, p) => sum + p.average * p.count, 0) / rated : 0
	);

	$effect(() => {
		const current = range;
		statsMood(current)
			.then((result) => {
				if (current === range) points = result;
			})
			.catch(async (e) => {
				if (!(await handleLocked(e))) error = errorText(e);
			});
	});
</script>

<UnlockedShell title={t('patterns.title')}>
	<div class="stack">
		<div class="segmented" role="radiogroup" aria-label={t('patterns.range')}>
			{#each RANGES as option (option.value)}
				<button
					type="button"
					role="radio"
					aria-checked={range === option.value}
					onclick={() => (range = option.value)}>{t(option.label)}</button
				>
			{/each}
		</div>

		<section class="card stack" aria-labelledby="mood-chart">
			<div>
				<h2 id="mood-chart">{t('patterns.chart')}</h2>
				<p class="muted small hint">{t('patterns.chartHint')}</p>
			</div>
			{#if error}
				<p class="notice">{error}</p>
			{:else if points && points.length === 0}
				<p class="muted">{t('patterns.empty')}</p>
			{:else if points}
				<p class="summary">
					{t('patterns.summary', {
						days: points.length,
						average: new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }).format(average)
					})}
				</p>
				<MoodChart {points} from={bounds[0]} to={bounds[1]} />
			{/if}
		</section>

		<p class="muted small">{t('patterns.note')}</p>
	</div>
</UnlockedShell>

<style>
	.segmented {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 4px;
		padding: 4px;
		background: var(--surface-2);
		border-radius: 999px;
	}
	.segmented button {
		min-height: 40px;
		padding: 0.4rem;
		color: var(--text-muted);
		background: transparent;
		font-size: 0.9rem;
	}
	.segmented button[aria-checked='true'] {
		color: var(--text);
		background: var(--surface);
		box-shadow: var(--shadow);
	}
	.hint {
		margin: 0.25rem 0 0;
	}
	.summary {
		margin: 0;
		font-weight: 600;
	}
</style>

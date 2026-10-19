<script lang="ts">
	// Mood over time (A-1): one series, 2px line, 8px+ markers with a surface
	// ring, hairline grid, crosshair + tooltip on hover and keyboard, table view.
	import type { MoodPoint } from '#lib/api/index.js';
	import { dayIndex, nearest, scale, xTicks } from '#lib/chart.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';

	let { points, from, to }: { points: MoodPoint[]; from: string; to: string } = $props();

	const HEIGHT = 250;
	// The top margin leaves room for the tooltip above the plot.
	const M = { top: 44, right: 14, bottom: 28, left: 28 };

	let width = $state(320);
	let active = $state<number | null>(null);

	let x = $derived(scale([0, dayIndex(from, to)], [M.left, width - M.right]));
	let y = $derived(scale([1, 5], [HEIGHT - M.bottom, M.top]));
	let xs = $derived(points.map((p) => x(dayIndex(from, p.date))));
	let path = $derived(
		points
			.map((p, i) => `${i === 0 ? 'M' : 'L'}${xs[i].toFixed(1)},${y(p.average).toFixed(1)}`)
			.join('')
	);
	let ticks = $derived(xTicks(from, to, width < 420 ? 5 : 7));

	let dayFormat = $derived(
		new Intl.DateTimeFormat(locale(), { day: 'numeric', month: 'short', timeZone: 'UTC' })
	);
	let numberFormat = $derived(new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }));
	const asDate = (day: string) => new Date(`${day}T00:00:00Z`);

	let tooltip = $derived(
		active === null
			? ''
			: t('patterns.tooltip', {
					date: dayFormat.format(asDate(points[active].date)),
					average: numberFormat.format(points[active].average),
					count: points[active].count
				})
	);

	function onpointermove(e: PointerEvent) {
		const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
		if (points.length) active = nearest(xs, e.clientX - rect.left);
	}

	function onkeydown(e: KeyboardEvent) {
		if (!points.length) return;
		if (e.key === 'ArrowRight') active = Math.min(points.length - 1, (active ?? -1) + 1);
		else if (e.key === 'ArrowLeft') active = Math.max(0, (active ?? points.length) - 1);
		else return;
		e.preventDefault();
	}
</script>

<div class="chart" bind:clientWidth={width}>
	<svg
		{width}
		height={HEIGHT}
		role="slider"
		aria-label={t('patterns.chart')}
		aria-valuemin={0}
		aria-valuemax={Math.max(0, points.length - 1)}
		aria-valuenow={active ?? points.length - 1}
		aria-valuetext={tooltip || t('patterns.chartHint')}
		tabindex="0"
		{onpointermove}
		onpointerleave={() => (active = null)}
		{onkeydown}
		onblur={() => (active = null)}
	>
		{#each [1, 2, 3, 4, 5] as level (level)}
			<line class="grid" x1={M.left} x2={width - M.right} y1={y(level)} y2={y(level)} />
			<text class="axis" x={M.left - 10} y={y(level)} dy="0.32em" text-anchor="end">{level}</text>
		{/each}
		{#each ticks as tick, i (tick)}
			{@const anchor =
				i === ticks.length - 1 ? 'end' : x(dayIndex(from, tick)) - M.left < 24 ? 'start' : 'middle'}
			<text class="axis" x={x(dayIndex(from, tick))} y={HEIGHT - 8} text-anchor={anchor}>
				{dayFormat.format(asDate(tick))}
			</text>
		{/each}

		{#if active !== null}
			<line class="crosshair" x1={xs[active]} x2={xs[active]} y1={M.top} y2={HEIGHT - M.bottom} />
		{/if}

		<path class="line" d={path} />
		{#each points as point, i (point.date)}
			<circle
				class="dot"
				class:active={i === active}
				cx={xs[i]}
				cy={y(point.average)}
				r={i === active ? 6 : 4.5}
			/>
		{/each}
	</svg>

	<div
		class="tooltip"
		class:visible={active !== null}
		aria-live="polite"
		style:left="{active === null ? 0 : Math.min(Math.max(xs[active], 80), width - 80)}px"
	>
		{tooltip}
	</div>
</div>

<details>
	<summary>{t('patterns.table')}</summary>
	<table>
		<thead>
			<tr
				><th scope="col">{t('patterns.date')}</th><th scope="col">{t('patterns.average')}</th><th
					scope="col">{t('patterns.count')}</th
				></tr
			>
		</thead>
		<tbody>
			{#each points as point (point.date)}
				<tr>
					<td>{dayFormat.format(asDate(point.date))}</td>
					<td>{numberFormat.format(point.average)}</td>
					<td>{point.count}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</details>

<style>
	.chart {
		position: relative;
		width: 100%;
	}
	svg {
		display: block;
		overflow: visible;
		touch-action: pan-y;
	}
	svg:focus-visible {
		outline-offset: 4px;
		border-radius: var(--radius-sm);
	}
	.grid {
		stroke: var(--chart-grid);
		stroke-width: 1;
	}
	.axis {
		fill: var(--text-muted);
		font-size: 0.72rem;
		font-variant-numeric: tabular-nums;
	}
	.crosshair {
		stroke: var(--text-muted);
		stroke-width: 1;
	}
	.line {
		fill: none;
		stroke: var(--chart-series);
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
	}
	.dot {
		fill: var(--chart-series);
		stroke: var(--surface);
		stroke-width: 2;
	}
	.tooltip {
		position: absolute;
		top: 0.25rem;
		transform: translateX(-50%);
		padding: 0.35rem 0.6rem;
		font-size: 0.8rem;
		font-weight: 600;
		white-space: nowrap;
		color: var(--text);
		background: var(--surface);
		border-radius: var(--radius-sm);
		box-shadow: var(--shadow-raised);
		opacity: 0;
		pointer-events: none;
		transition: opacity 0.1s;
	}
	.tooltip.visible {
		opacity: 1;
	}
	details {
		margin-top: 0.75rem;
	}
	summary {
		min-height: var(--tap);
		display: flex;
		align-items: center;
		color: var(--accent);
		font-weight: 600;
		cursor: pointer;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-variant-numeric: tabular-nums;
	}
	th,
	td {
		padding: 0.4rem 0.25rem;
		text-align: left;
		border-bottom: 1px solid var(--border);
	}
	th {
		font-size: 0.8rem;
		color: var(--text-muted);
		font-weight: 600;
	}
</style>

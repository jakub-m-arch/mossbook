<script lang="ts">
	// Labeled horizontal bars (A-2–A-4): ≤ 24px thick, rounded data end, value
	// and count in text tokens beside the bar (dataviz mark specs).
	export interface Bar {
		key: string;
		label: string;
		/** Bar length relative to `max`. */
		value: number;
		/** Text shown at the end, e.g. "3,5". */
		display: string;
		/** Secondary text, e.g. "days: 4". */
		note: string;
	}

	let { bars, max, label }: { bars: Bar[]; max: number; label: string } = $props();
</script>

<ul class="bars" aria-label={label}>
	{#each bars as bar (bar.key)}
		<li>
			<div class="row">
				<span class="label">{bar.label}</span>
				<span class="value">{bar.display}</span>
			</div>
			<div class="track" aria-hidden="true">
				<div
					class="fill"
					style:width="{Math.max(2, Math.min(100, (bar.value / max) * 100))}%"
				></div>
			</div>
			<span class="note">{bar.note}</span>
		</li>
	{/each}
</ul>

<style>
	.bars {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.row {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		font-size: 0.9rem;
	}
	.label {
		overflow-wrap: anywhere;
	}
	.value {
		font-weight: 650;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.track {
		height: 10px;
		margin-top: 0.3rem;
		border-radius: 0 4px 4px 0;
		background: var(--chart-grid);
	}
	.fill {
		height: 100%;
		border-radius: 0 4px 4px 0;
		background: var(--chart-series);
	}
	.note {
		font-size: 0.75rem;
		color: var(--text-muted);
	}
</style>

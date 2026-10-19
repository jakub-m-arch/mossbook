<script lang="ts">
	// Weather pictograms for the 1–5 rating (E-1: pictograms, not emoji faces).
	let { value, size = 32 }: { value: number; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true" class="mood">
	{#if value >= 4}
		<circle cx={value === 5 ? 16 : 12} cy={value === 5 ? 16 : 12} r="6" class="sun" />
		{#if value === 5}
			{#each [0, 45, 90, 135, 180, 225, 270, 315] as angle (angle)}
				<line x1="16" y1="3" x2="16" y2="6.5" class="sun-ray" transform="rotate({angle} 16 16)" />
			{/each}
		{/if}
	{/if}
	{#if value <= 4}
		<path
			d="M9 24h15a5 5 0 0 0 0-10 7 7 0 0 0-13.5 1.5A4.3 4.3 0 0 0 9 24z"
			class={value <= 2 ? 'cloud dark' : 'cloud'}
		/>
	{/if}
	{#if value <= 2}
		<line x1="12" y1="26.5" x2="11" y2="29.5" class="rain" />
		<line x1="17" y1="26.5" x2="16" y2="29.5" class="rain" />
		{#if value === 1}
			<line x1="22" y1="26.5" x2="21" y2="29.5" class="rain" />
		{/if}
	{/if}
</svg>

<style>
	.mood {
		flex: none;
	}
	.sun {
		fill: #e3b04b;
	}
	.sun-ray {
		stroke: #e3b04b;
		stroke-width: 2;
		stroke-linecap: round;
	}
	.cloud {
		fill: var(--surface-2);
		stroke: var(--text-muted);
		stroke-width: 1.5;
	}
	.cloud.dark {
		fill: var(--border);
	}
	.rain {
		stroke: var(--focus);
		stroke-width: 2;
		stroke-linecap: round;
	}
</style>

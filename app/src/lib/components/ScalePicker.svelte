<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';

	interface Props {
		label: string;
		value: number | null | undefined;
		min?: number;
		max?: number;
		name: string;
	}

	let { label, value = $bindable(), min = 0, max = 10, name }: Props = $props();
	let levels = $derived(Array.from({ length: max - min + 1 }, (_, i) => min + i));
</script>

<fieldset>
	<legend>{label}</legend>
	<div class="options" style:--count={levels.length}>
		{#each levels as level (level)}
			<label class="option" class:selected={value === level}>
				<input
					type="radio"
					{name}
					value={level}
					checked={value === level}
					onclick={() => (value = value === level ? null : level)}
					class="sr-only"
				/>
				{level}
			</label>
		{/each}
	</div>
	{#if value !== null && value !== undefined}
		<button type="button" class="link" onclick={() => (value = null)}>{t('scale.clear')}</button>
	{/if}
</fieldset>

<style>
	fieldset {
		border: 0;
		margin: 0;
		padding: 0;
	}
	legend {
		font-size: 0.9rem;
		font-weight: 600;
		margin-bottom: 0.5rem;
	}
	.options {
		display: grid;
		grid-template-columns: repeat(min(var(--count), 6), 1fr);
		gap: 0.35rem;
	}
	.option {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.2rem;
		min-height: var(--tap);
		padding: 0.4rem 0.2rem;
		font-size: 0.8rem;
		font-weight: 600;
		text-align: center;
		line-height: 1.2;
		background: var(--surface-2);
		border: 1.5px solid transparent;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-variant-numeric: tabular-nums;
		transition: background 0.15s;
	}
	.option:hover {
		background: var(--surface-3);
	}
	.option.selected {
		color: var(--accent);
		background: var(--accent-soft);
		border-color: var(--accent);
	}
	.option:focus-within {
		outline: 3px solid var(--focus);
		outline-offset: 2px;
	}
</style>

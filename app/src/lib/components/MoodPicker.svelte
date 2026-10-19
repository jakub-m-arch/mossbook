<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';

	import MoodIcon from './MoodIcon.svelte';

	let { value = $bindable(), name = 'mood' }: { value: number | null | undefined; name?: string } =
		$props();
</script>

<fieldset>
	<legend>{t('mood.label')}</legend>
	<div class="options">
		{#each [1, 2, 3, 4, 5] as level (level)}
			<label class="option" class:selected={value === level}>
				<input
					type="radio"
					{name}
					value={level}
					checked={value === level}
					onclick={() => (value = value === level ? null : level)}
					class="sr-only"
				/>
				<MoodIcon value={level} />
				<span>{t(`mood.${level}`)}</span>
			</label>
		{/each}
	</div>
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
		grid-template-columns: repeat(5, 1fr);
		gap: 0.4rem;
	}
	.option {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.2rem;
		min-height: 4.75rem;
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

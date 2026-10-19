<script lang="ts" generics="Key extends string">
	import { t } from '#lib/i18n/index.svelte.js';

	type Item = Key | { custom: string };

	interface Props {
		label: string;
		/** Translation prefix, e.g. `emotion` for `emotion.calm`. */
		prefix: string;
		options: readonly Key[];
		selected: Item[];
	}

	let { label, prefix, options, selected = $bindable() }: Props = $props();
	let custom = $state('');

	const isOn = (key: Key) => selected.includes(key);

	function toggle(key: Key) {
		selected = isOn(key) ? selected.filter((i) => i !== key) : [...selected, key];
	}

	function addCustom() {
		const text = custom.trim();
		if (!text) return;
		if (!selected.some((i) => typeof i !== 'string' && i.custom === text)) {
			selected = [...selected, { custom: text }];
		}
		custom = '';
	}

	function removeCustom(text: string) {
		selected = selected.filter((i) => typeof i === 'string' || i.custom !== text);
	}
</script>

<fieldset>
	<legend>{label}</legend>
	<div class="chips">
		{#each options as key (key)}
			<button type="button" class="chip" aria-pressed={isOn(key)} onclick={() => toggle(key)}>
				{t(`${prefix}.${key}`)}
			</button>
		{/each}
		{#each selected as item (typeof item === 'string' ? item : `custom:${item.custom}`)}
			{#if typeof item !== 'string'}
				<button
					type="button"
					class="chip"
					aria-pressed="true"
					onclick={() => removeCustom(item.custom)}
				>
					{item.custom} ×
				</button>
			{/if}
		{/each}
	</div>
	<div class="add">
		<input
			bind:value={custom}
			placeholder={t('custom.placeholder')}
			aria-label={`${label}: ${t('custom.add')}`}
			onkeydown={(e) => {
				if (e.key === 'Enter') {
					e.preventDefault();
					addCustom();
				}
			}}
		/>
		<button type="button" class="secondary" onclick={addCustom}>{t('custom.add')}</button>
	</div>
</fieldset>

<style>
	fieldset {
		border: 0;
		margin: 0;
		padding: 0;
	}
	legend {
		font-weight: 600;
		margin-bottom: 0.4rem;
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
	}
	.chip {
		min-height: 40px;
		padding: 0.3rem 0.8rem;
		font-weight: 500;
		color: var(--text);
		background: var(--surface);
		border: 2px solid var(--border);
		border-radius: 999px;
	}
	.chip[aria-pressed='true'] {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.add {
		display: flex;
		gap: 0.5rem;
		margin-top: 0.5rem;
	}
</style>

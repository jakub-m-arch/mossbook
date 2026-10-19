<script lang="ts">
	import type { Id, Tag } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';

	let { tags, selected = $bindable() }: { tags: Tag[]; selected: Id[] } = $props();

	function toggle(id: Id) {
		selected = selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id];
	}
</script>

{#if tags.length > 0}
	<fieldset>
		<legend>{t('field.tags')}</legend>
		<div class="chips">
			{#each tags as tag (tag.id)}
				<button
					type="button"
					class="chip"
					aria-pressed={selected.includes(tag.id)}
					onclick={() => toggle(tag.id)}
				>
					<span class="dot" style:background={tag.color}></span>{tag.name}
				</button>
			{/each}
		</div>
	</fieldset>
{/if}

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
		gap: 0.4rem;
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
	.dot {
		width: 0.7rem;
		height: 0.7rem;
		border-radius: 50%;
	}
</style>

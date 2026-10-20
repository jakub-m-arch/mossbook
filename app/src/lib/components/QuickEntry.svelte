<script lang="ts">
	// E-10: at most two taps (type, then rating or intensity). The entry is saved
	// as "to complete" right away; the caller then opens its form so details can
	// be added now or later.
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';

	import { quickEntry, quickEntryLocked } from '#lib/api/index.js';
	import type { Entry, EntryKindTag } from '#lib/api/index.js';
	import { CREATABLE_KINDS } from '#lib/entries.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	import KindIcon from './KindIcon.svelte';
	import MoodIcon from './MoodIcon.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		/** From the lock screen (E-15): saved to the inbox, no form afterwards. */
		locked?: boolean;
		onsaved: (entry: Entry | null) => void;
	}

	let { locked = false, onsaved }: Props = $props();

	let sheet: Sheet;
	let kind = $state<EntryKindTag | null>(null);
	let error = $state('');
	let busy = $state(false);

	/** Kinds saved with the first tap: they have no rating to ask for. */
	const ONE_TAP: EntryKindTag[] = ['sleep', 'nightmare', 'note'];

	export function open() {
		kind = null;
		error = '';
		sheet.open();
	}

	async function save(tag: EntryKindTag, value: number | null) {
		if (busy) return;
		busy = true;
		error = '';
		try {
			if (locked) {
				await quickEntryLocked(tag, value);
				sheet.close();
				onsaved(null);
			} else {
				const entry = await quickEntry(tag, value);
				sheet.close();
				onsaved(entry);
			}
		} catch (e) {
			if (locked || !(await handleLocked(e))) error = errorText(e);
		} finally {
			busy = false;
		}
	}

	function choose(tag: EntryKindTag) {
		if (ONE_TAP.includes(tag)) void save(tag, null);
		else kind = tag;
	}
</script>

<Sheet
	bind:this={sheet}
	title={kind ? t(`kind.${kind}`) : t('quick.chooseKind')}
	onclose={() => (kind = null)}
>
	{#if !kind}
		{#if locked}<p class="muted small hint">{t('quick.lockedHint')}</p>{/if}
		<div class="kinds">
			{#each CREATABLE_KINDS as tag (tag)}
				<button type="button" class="kind" onclick={() => choose(tag)} disabled={busy}>
					<KindIcon kind={tag} size={44} />
					<span>{t(`kind.${tag}`)}</span>
				</button>
			{/each}
		</div>
	{:else}
		<p class="muted prompt">{kind === 'mood' ? t('mood.label') : t('quick.chooseValue')}</p>
		{#if kind === 'mood'}
			<div class="moods">
				{#each [1, 2, 3, 4, 5] as level (level)}
					<button
						type="button"
						class="option mood"
						disabled={busy}
						onclick={() => save('mood', level)}
					>
						<MoodIcon value={level} size={34} />
						<span>{t(`mood.${level}`)}</span>
					</button>
				{/each}
			</div>
		{:else}
			<div class="scale" role="group" aria-label={t('scale.intensity')}>
				{#each Array.from({ length: 11 }, (_, i) => i) as level (level)}
					<button
						type="button"
						class="option"
						disabled={busy}
						onclick={() => kind && save(kind, level)}>{level}</button
					>
				{/each}
			</div>
		{/if}
		<div class="actions">
			<button type="button" class="link" onclick={() => (kind = null)}>
				<ChevronLeft size={18} />{t('common.back')}
			</button>
			<button type="button" class="link" disabled={busy} onclick={() => kind && save(kind, null)}>
				{t('quick.saveWithout')}
			</button>
		</div>
	{/if}
	{#if error}<p class="notice" aria-live="polite">{error}</p>{/if}
</Sheet>

<style>
	.kinds {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
		gap: 0.6rem;
	}
	.kind {
		flex-direction: column;
		gap: 0.5rem;
		min-height: 6.5rem;
		padding: 0.9rem 0.5rem;
		color: var(--text);
		background: var(--surface-2);
		border-radius: var(--radius);
		font-weight: 600;
	}
	.kind:hover:not(:disabled) {
		background: var(--surface-3);
	}
	.hint {
		margin: 0;
	}
	.prompt {
		margin: 0;
	}
	.option {
		color: var(--text);
		background: var(--surface-2);
		border-radius: var(--radius-sm);
		padding: 0;
		font-variant-numeric: tabular-nums;
	}
	.option:hover:not(:disabled) {
		color: var(--accent);
		background: var(--accent-soft);
	}
	.moods {
		display: grid;
		grid-template-columns: repeat(5, 1fr);
		gap: 0.4rem;
	}
	.mood {
		flex-direction: column;
		gap: 0.3rem;
		min-height: 5.5rem;
		padding: 0.5rem 0.2rem;
		font-size: 0.75rem;
		font-weight: 500;
		line-height: 1.2;
	}
	.scale {
		display: grid;
		grid-template-columns: repeat(6, 1fr);
		gap: 0.4rem;
	}
	.actions {
		display: flex;
		justify-content: space-between;
	}
</style>

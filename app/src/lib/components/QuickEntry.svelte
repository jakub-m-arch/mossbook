<script lang="ts">
	// E-10: at most two taps (type, then rating or intensity). The entry is saved
	// as "to complete" and can be filled in later.
	import { resolve } from '$app/paths';
	import { quickEntry } from '#lib/api/index.js';
	import type { EntryKindTag } from '#lib/api/index.js';
	import { CREATABLE_KINDS } from '#lib/entries.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	import MoodIcon from './MoodIcon.svelte';

	let { onsaved }: { onsaved: () => void } = $props();

	let dialog: HTMLDialogElement;
	let kind = $state<EntryKindTag | null>(null);
	let error = $state('');
	let busy = $state(false);

	/** Kinds saved with the first tap: they have no rating to ask for. */
	const ONE_TAP: EntryKindTag[] = ['sleep', 'note'];

	export function open() {
		kind = null;
		error = '';
		dialog.showModal();
	}

	async function save(tag: EntryKindTag, value: number | null) {
		if (busy) return;
		busy = true;
		error = '';
		try {
			await quickEntry(tag, value);
			dialog.close();
			onsaved();
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		} finally {
			busy = false;
		}
	}

	function choose(tag: EntryKindTag) {
		if (ONE_TAP.includes(tag)) void save(tag, null);
		else kind = tag;
	}
</script>

<dialog bind:this={dialog} aria-labelledby="quick-title" onclose={() => (kind = null)}>
	<div class="stack">
		<h2 id="quick-title">{kind ? t(`kind.${kind}`) : t('quick.chooseKind')}</h2>

		{#if !kind}
			<div class="grid">
				{#each CREATABLE_KINDS as tag (tag)}
					<button type="button" class="secondary big" onclick={() => choose(tag)} disabled={busy}>
						{t(`kind.${tag}`)}
					</button>
				{/each}
			</div>
		{:else}
			<p class="muted">{t('quick.chooseValue')}</p>
			{#if kind === 'mood'}
				<div class="moods">
					{#each [1, 2, 3, 4, 5] as level (level)}
						<button
							type="button"
							class="secondary mood"
							disabled={busy}
							onclick={() => save('mood', level)}
						>
							<MoodIcon value={level} />
							<span>{t(`mood.${level}`)}</span>
						</button>
					{/each}
				</div>
			{:else}
				<div class="scale" role="group" aria-label={t('scale.intensity')}>
					{#each Array.from({ length: 11 }, (_, i) => i) as level (level)}
						<button
							type="button"
							class="secondary"
							disabled={busy}
							onclick={() => kind && save(kind, level)}>{level}</button
						>
					{/each}
				</div>
			{/if}
			<button type="button" class="link" disabled={busy} onclick={() => kind && save(kind, null)}>
				{t('quick.saveWithout')}
			</button>
			<a class="button secondary" href={resolve(`/entry/new?kind=${kind}`)}>{t('quick.more')}</a>
		{/if}

		{#if error}<p class="notice" aria-live="polite">{error}</p>{/if}
		<button type="button" class="secondary" onclick={() => dialog.close()}>
			{t('common.cancel')}
		</button>
	</div>
</dialog>

<style>
	dialog {
		width: min(32rem, calc(100vw - 2rem));
		padding: 1.25rem;
		color: var(--text);
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	dialog::backdrop {
		background: rgb(0 0 0 / 0.35);
	}
	h2 {
		margin: 0;
		font-size: 1.3rem;
	}
	.grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}
	.big {
		min-height: 4rem;
	}
	.moods {
		display: grid;
		grid-template-columns: repeat(5, 1fr);
		gap: 0.4rem;
	}
	.mood {
		flex-direction: column;
		gap: 0.2rem;
		min-height: 4.5rem;
		padding: 0.4rem 0.2rem;
		font-size: 0.75rem;
		font-weight: 500;
	}
	.scale {
		display: grid;
		grid-template-columns: repeat(6, 1fr);
		gap: 0.35rem;
	}
</style>

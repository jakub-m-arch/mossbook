<script lang="ts">
	import { onMount } from 'svelte';

	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { entryDelete, entryGet, entrySave, tagsList } from '#lib/api/index.js';
	import type { Entry, EntryDraft, Tag } from '#lib/api/index.js';
	import ConfirmAction from '#lib/components/ConfirmAction.svelte';
	import EntryForm from '#lib/components/EntryForm.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { draftFromEntry, kindTag } from '#lib/entries.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	let entry = $state<Entry | null>(null);
	let tags = $state<Tag[]>([]);
	let missing = $state(false);
	let error = $state('');

	onMount(async () => {
		try {
			[entry, tags] = await Promise.all([entryGet(page.params.id ?? ''), tagsList()]);
		} catch (e) {
			if (!(await handleLocked(e))) missing = true;
		}
	});

	async function save(draft: EntryDraft) {
		error = '';
		try {
			await entrySave(draft);
			await goto(resolve('/timeline'));
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}

	async function remove() {
		if (!entry) return;
		try {
			await entryDelete(entry.id);
			await goto(resolve('/timeline'), { replaceState: true });
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}
</script>

<UnlockedShell title={entry ? t(`kind.${kindTag(entry.kind)}`) : t('entry.edit')} back>
	{#if missing}<p class="muted">{t('entry.notFound')}</p>{/if}
	{#if error}<p class="notice" aria-live="polite">{error}</p>{/if}
	{#if entry}
		{#key entry.id}
			<EntryForm initial={draftFromEntry(entry)} {tags} onsave={save} />
		{/key}
		<div class="danger">
			<ConfirmAction
				label={t('entry.delete')}
				question={t('entry.deleteConfirm')}
				onconfirm={remove}
			/>
		</div>
	{/if}
</UnlockedShell>

<style>
	.danger {
		margin-top: 2rem;
	}
</style>

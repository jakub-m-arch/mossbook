<script lang="ts">
	import { onMount } from 'svelte';

	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { entrySave, tagsList } from '#lib/api/index.js';
	import type { EntryDraft, EntryKindTag, Tag } from '#lib/api/index.js';
	import EntryForm from '#lib/components/EntryForm.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { CREATABLE_KINDS, newDraft } from '#lib/entries.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	const requested = page.url.searchParams.get('kind') as EntryKindTag | null;
	const kind: EntryKindTag = requested && CREATABLE_KINDS.includes(requested) ? requested : 'note';

	let tags = $state<Tag[] | null>(null);
	let error = $state('');

	onMount(async () => {
		tags = await tagsList().catch(() => []);
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
</script>

<UnlockedShell title={t(`kind.${kind}`)} back>
	{#if error}<p class="notice" aria-live="polite">{error}</p>{/if}
	{#if tags}
		<EntryForm initial={newDraft(kind)} {tags} onsave={save} />
	{/if}
</UnlockedShell>

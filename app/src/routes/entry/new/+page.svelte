<script lang="ts">
	import { onMount } from 'svelte';

	import { beforeNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { entrySave, tagsList, unsavedGet } from '#lib/api/index.js';
	import type { EntryDraft, EntryKindTag, Tag } from '#lib/api/index.js';
	import EntryForm from '#lib/components/EntryForm.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { ALL_KINDS, kindTag, newDraft } from '#lib/entries.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';
	import { keepUnsaved, leaveForm } from '#lib/unsaved.js';

	const requested = page.url.searchParams.get('kind') as EntryKindTag | null;
	const kind: EntryKindTag = requested && ALL_KINDS.includes(requested) ? requested : 'note';
	// "Add a question" on the session screen (D-26): a note marked for the session.
	const discuss = page.url.searchParams.get('discuss') === '1';
	const back = page.url.searchParams.get('from') === 'session' ? '/session' : '/timeline';

	// Continues the form kept when the journal locked (D-20).
	const resume = page.state.resume === true;

	let tags = $state<Tag[] | null>(null);
	let initial = $state<EntryDraft>(newDraft(kind, { discuss }));
	let error = $state('');

	onMount(async () => {
		const [list, kept] = await Promise.all([
			tagsList().catch(() => []),
			resume ? unsavedGet().catch(() => null) : null
		]);
		if (kept && !kept.draft.id && kindTag(kept.draft.kind) === kind) initial = kept.draft;
		tags = list;
	});

	beforeNavigate(({ to }) => leaveForm(to));

	async function save(draft: EntryDraft) {
		error = '';
		try {
			await entrySave(draft);
			await goto(resolve(back));
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}
</script>

<UnlockedShell title={t(`kind.${kind}`)} {back}>
	{#if error}<p class="notice" aria-live="polite">{error}</p>{/if}
	{#if tags}
		<EntryForm {initial} {tags} onsave={save} onchange={keepUnsaved} />
	{/if}
</UnlockedShell>

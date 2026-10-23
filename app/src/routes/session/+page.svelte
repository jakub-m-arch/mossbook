<script lang="ts">
	// For the next therapy session (E-7, E-20, D-26): what to do from the latest
	// session, the entries flagged for the next one, a question to add, and the
	// notes to write after it.
	import MessageCirclePlus from '@lucide/svelte/icons/message-circle-plus';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import { onMount } from 'svelte';

	import { resolve } from '$app/paths';
	import { entriesList, entrySave, tagsList } from '#lib/api/index.js';
	import type { Entry, Tag } from '#lib/api/index.js';
	import EntryCard from '#lib/components/EntryCard.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { draftFromEntry } from '#lib/entries.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { JOURNAL_CHANGED, errorText, handleLocked } from '#lib/session.svelte.js';

	let topics = $state<Entry[] | null>(null);
	let last = $state<Entry | null>(null);
	let tags = $state<Tag[]>([]);
	let error = $state('');
	let message = $state('');

	let tagMap = $derived(new Map(tags.map((tag) => [tag.id, tag])));
	let todo = $derived(
		last && typeof last.kind !== 'string' && 'session' in last.kind
			? last.kind.session.todo.trim()
			: ''
	);
	let dateFormat = $derived(new Intl.DateTimeFormat(locale(), { dateStyle: 'long' }));

	async function load() {
		try {
			const [flagged, sessions] = await Promise.all([
				entriesList({ discuss_only: true }),
				entriesList({ kinds: ['session'] })
			]);
			topics = flagged;
			last = sessions[0] ?? null;
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}

	onMount(() => {
		void load();
		tagsList()
			.then((list) => (tags = list))
			.catch(() => {});
		window.addEventListener(JOURNAL_CHANGED, load);
		return () => window.removeEventListener(JOURNAL_CHANGED, load);
	});

	async function unflag(entry: Entry) {
		message = '';
		try {
			await entrySave({ ...draftFromEntry(entry), incomplete: entry.incomplete, discuss: false });
			message = t('session.unflagged');
			await load();
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}
</script>

<UnlockedShell title={t('session.title')} back="/timeline">
	<div class="stack">
		<p class="muted">{t('session.intro')}</p>
		{#if error}<p class="notice">{error}</p>{/if}

		{#if last && todo}
			<section class="card stack" aria-labelledby="todo-title">
				<h2 id="todo-title">{t('session.todoTitle')}</h2>
				<p class="todo">{todo}</p>
				<a class="muted small" href={resolve('/entry/[id]', { id: last.id })}>
					{t('session.todoFrom', { date: dateFormat.format(new Date(last.occurred_at)) })}
				</a>
			</section>
		{/if}

		<section class="stack" aria-labelledby="topics-title">
			<h2 id="topics-title">{t('session.topics')}</h2>
			{#if topics && topics.length === 0}
				<p class="muted">{t('session.topicsEmpty')}</p>
			{:else if topics}
				<ul class="list">
					{#each topics as entry (entry.id)}
						<li class="topic">
							<EntryCard {entry} tags={tagMap} />
							<button type="button" class="link" onclick={() => unflag(entry)}>
								{t('session.unflag')}
							</button>
						</li>
					{/each}
				</ul>
			{/if}
			{#if message}<p class="muted small" role="status">{message}</p>{/if}
		</section>

		<div class="actions">
			<a class="button secondary" href={resolve('/entry/new?kind=note&discuss=1&from=session')}>
				<MessageCirclePlus size={20} />{t('session.addQuestion')}
			</a>
			<a class="button" href={resolve('/entry/new?kind=session&from=session')}>
				<NotebookPen size={20} />{t('session.write')}
			</a>
		</div>
	</div>
</UnlockedShell>

<style>
	.todo {
		margin: 0;
		white-space: pre-wrap;
	}
	.list {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.topic {
		display: grid;
		gap: 0.1rem;
	}
	.topic .link {
		justify-self: end;
	}
	.actions {
		display: grid;
		gap: 0.5rem;
	}
</style>

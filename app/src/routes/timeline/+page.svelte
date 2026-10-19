<script lang="ts">
	import { onMount } from 'svelte';

	import { resolve } from '$app/paths';
	import { entriesList, tagsList } from '#lib/api/index.js';
	import type { Entry, EntryFilter, EntryKindTag, Tag } from '#lib/api/index.js';
	import EntryCard from '#lib/components/EntryCard.svelte';
	import QuickEntry from '#lib/components/QuickEntry.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { CREATABLE_KINDS } from '#lib/entries.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';
	import { endOfDay, startOfDay } from '#lib/time.js';

	let entries = $state<Entry[]>([]);
	let tags = $state<Tag[]>([]);
	let loaded = $state(false);
	let error = $state('');
	let saved = $state(false);
	let quick: QuickEntry;

	// Filters (E-13); search runs in Rust over decrypted entries.
	let text = $state('');
	let kinds = $state<EntryKindTag[]>([]);
	let tagFilter = $state<string[]>([]);
	let from = $state('');
	let to = $state('');
	let incompleteOnly = $state(false);

	let tagMap = $derived(new Map(tags.map((tag) => [tag.id, tag])));
	let filtered = $derived(
		text.trim() !== '' ||
			kinds.length > 0 ||
			tagFilter.length > 0 ||
			from !== '' ||
			to !== '' ||
			incompleteOnly
	);

	function filter(): EntryFilter {
		return {
			text: text.trim() || undefined,
			kinds,
			tags: tagFilter,
			from: from ? startOfDay(from) : undefined,
			to: to ? endOfDay(to) : undefined,
			incomplete_only: incompleteOnly
		};
	}

	let request = 0;
	async function load() {
		const id = ++request;
		try {
			const result = await entriesList(filter());
			if (id === request) {
				entries = result;
				loaded = true;
				error = '';
			}
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}

	// Reload when a filter changes; typing in the search box is debounced.
	$effect(() => {
		void [text, kinds.length, tagFilter.length, from, to, incompleteOnly];
		const timer = setTimeout(load, 200);
		return () => clearTimeout(timer);
	});

	onMount(async () => {
		tags = await tagsList().catch(() => []);
	});

	const toggle = <T,>(list: T[], item: T) =>
		list.includes(item) ? list.filter((i) => i !== item) : [...list, item];

	function clearFilters() {
		text = from = to = '';
		kinds = [];
		tagFilter = [];
		incompleteOnly = false;
	}

	function onQuickSaved() {
		saved = true;
		setTimeout(() => (saved = false), 4000);
		void load();
	}
</script>

<UnlockedShell title={t('timeline.title')}>
	<div class="stack">
		<button type="button" class="quick" onclick={() => quick.open()}>{t('quick.open')}</button>
		<!-- P-2: the plan is one tap away from the home screen. -->
		<a class="button secondary" href={resolve('/plan')}>{t('plan.title')}</a>
		{#if saved}<p class="notice" role="status">{t('quick.done')}</p>{/if}

		<div class="filters stack">
			<div>
				<label for="search" class="sr-only">{t('filter.search')}</label>
				<input id="search" type="search" placeholder={t('filter.search')} bind:value={text} />
			</div>
			<div class="chips" role="group" aria-label={t('filter.kinds')}>
				{#each CREATABLE_KINDS as kind (kind)}
					<button
						type="button"
						class="chip"
						aria-pressed={kinds.includes(kind)}
						onclick={() => (kinds = toggle(kinds, kind))}>{t(`kind.${kind}`)}</button
					>
				{/each}
			</div>
			{#if tags.length > 0}
				<div class="chips" role="group" aria-label={t('filter.tags')}>
					{#each tags as tag (tag.id)}
						<button
							type="button"
							class="chip"
							aria-pressed={tagFilter.includes(tag.id)}
							onclick={() => (tagFilter = toggle(tagFilter, tag.id))}
						>
							<span class="dot" style:background={tag.color}></span>{tag.name}
						</button>
					{/each}
				</div>
			{/if}
			<details>
				<summary>{t('filter.more')}</summary>
				<div class="stack more">
					<div class="dates">
						<div>
							<label for="from">{t('filter.from')}</label>
							<input id="from" type="date" bind:value={from} />
						</div>
						<div>
							<label for="to">{t('filter.to')}</label>
							<input id="to" type="date" bind:value={to} />
						</div>
					</div>
					<label class="check">
						<input type="checkbox" bind:checked={incompleteOnly} />
						{t('filter.incomplete')}
					</label>
				</div>
			</details>
			{#if filtered}
				<button type="button" class="link" onclick={clearFilters}>{t('filter.clear')}</button>
			{/if}
		</div>

		{#if error}<p class="notice">{error}</p>{/if}

		{#if loaded && entries.length === 0}
			<p class="muted">{filtered ? t('filter.none') : t('timeline.empty')}</p>
		{/if}
		<ul class="list" aria-label={t('timeline.title')}>
			{#each entries as entry (entry.id)}
				<li><EntryCard {entry} tags={tagMap} /></li>
			{/each}
		</ul>

		<div class="links">
			<a class="button secondary" href={resolve('/tags')}>{t('tags.manage')}</a>
			<a class="button secondary" href={resolve('/settings')}>{t('settings.open')}</a>
		</div>
	</div>
</UnlockedShell>

<QuickEntry bind:this={quick} onsaved={onQuickSaved} />

<style>
	.quick {
		min-height: 4rem;
		font-size: 1.15rem;
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
		width: 0.6rem;
		height: 0.6rem;
		border-radius: 50%;
	}
	details summary {
		min-height: var(--tap);
		display: flex;
		align-items: center;
		cursor: pointer;
		color: var(--accent);
		font-weight: 500;
	}
	.more {
		padding-top: 0.5rem;
	}
	.dates {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		font-weight: 500;
	}
	.check input {
		width: 1.4rem;
		min-height: 1.4rem;
	}
	.links {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}
	.list {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
</style>

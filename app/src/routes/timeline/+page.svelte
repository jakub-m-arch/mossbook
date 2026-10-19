<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import Search from '@lucide/svelte/icons/search';
	import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
	import Sprout from '@lucide/svelte/icons/sprout';
	import { onMount } from 'svelte';

	import { entriesList, tagsList } from '#lib/api/index.js';
	import type { Entry, EntryFilter, EntryKindTag, Tag } from '#lib/api/index.js';
	import EntryCard from '#lib/components/EntryCard.svelte';
	import QuickEntry from '#lib/components/QuickEntry.svelte';
	import Sheet from '#lib/components/Sheet.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { CREATABLE_KINDS, groupByDay } from '#lib/entries.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';
	import { endOfDay, startOfDay } from '#lib/time.js';

	let entries = $state<Entry[]>([]);
	let tags = $state<Tag[]>([]);
	let loaded = $state(false);
	let error = $state('');
	let saved = $state(false);
	let quick: QuickEntry;
	let filters: Sheet;

	// Filters (E-13); search runs in Rust over decrypted entries.
	let text = $state('');
	let kinds = $state<EntryKindTag[]>([]);
	let tagFilter = $state<string[]>([]);
	let from = $state('');
	let to = $state('');
	let incompleteOnly = $state(false);

	let tagMap = $derived(new Map(tags.map((tag) => [tag.id, tag])));
	let filterCount = $derived(
		kinds.length + tagFilter.length + (from ? 1 : 0) + (to ? 1 : 0) + (incompleteOnly ? 1 : 0)
	);
	let filtered = $derived(text.trim() !== '' || filterCount > 0);
	let groups = $derived(groupByDay(entries));
	let dayFormat = $derived(
		new Intl.DateTimeFormat(locale(), { weekday: 'long', day: 'numeric', month: 'long' })
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
	{#snippet fab()}
		<button type="button" class="fab" onclick={() => quick.open()} aria-label={t('quick.open')}>
			<Plus size={28} strokeWidth={2.4} />
		</button>
	{/snippet}

	<div class="stack">
		<div class="toolbar">
			<label class="search">
				<Search size={18} />
				<span class="sr-only">{t('filter.search')}</span>
				<input type="search" placeholder={t('filter.search')} bind:value={text} />
			</label>
			<button
				type="button"
				class="secondary filter-button"
				class:active={filterCount > 0}
				onclick={() => filters.open()}
				aria-label={filterCount > 0
					? t('filter.active', { count: filterCount })
					: t('filter.button')}
			>
				<SlidersHorizontal size={18} />
				{#if filterCount > 0}<span class="count">{filterCount}</span>{/if}
			</button>
		</div>

		{#if saved}<p class="notice" role="status">{t('quick.done')}</p>{/if}
		{#if error}<p class="notice">{error}</p>{/if}

		{#if loaded && entries.length === 0}
			{#if filtered}
				<div class="empty">
					<p class="muted">{t('filter.none')}</p>
					<button type="button" class="link" onclick={clearFilters}>{t('filter.clear')}</button>
				</div>
			{:else}
				<div class="empty">
					<span class="empty-icon"><Sprout size={32} /></span>
					<h2>{t('timeline.emptyTitle')}</h2>
					<p class="muted">{t('timeline.emptyHint')}</p>
					<p class="sr-only">{t('timeline.empty')}</p>
				</div>
			{/if}
		{/if}

		{#each groups as group (group.key)}
			<section class="day" aria-label={group.relative ? t(`day.${group.relative}`) : group.key}>
				<h2 class="eyebrow">
					{group.relative
						? t(`day.${group.relative}`)
						: dayFormat.format(new Date(`${group.key}T12:00`))}
				</h2>
				<ul class="list">
					{#each group.items as entry (entry.id)}
						<li><EntryCard {entry} tags={tagMap} /></li>
					{/each}
				</ul>
			</section>
		{/each}
	</div>
</UnlockedShell>

<QuickEntry bind:this={quick} onsaved={onQuickSaved} />

<Sheet bind:this={filters} title={t('filter.button')}>
	<div class="stack">
		<div>
			<p class="eyebrow">{t('filter.kinds')}</p>
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
		</div>
		{#if tags.length > 0}
			<div>
				<p class="eyebrow">{t('filter.tags')}</p>
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
			</div>
		{/if}
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
		<div class="sheet-actions">
			<button type="button" class="link" onclick={clearFilters} disabled={!filtered}
				>{t('filter.clear')}</button
			>
			<button type="button" onclick={() => filters.close()}>{t('filter.apply')}</button>
		</div>
	</div>
</Sheet>

<style>
	.toolbar {
		display: flex;
		gap: 0.5rem;
	}
	.search {
		flex: 1;
		position: relative;
		margin: 0;
		color: var(--text-muted);
	}
	.search :global(svg) {
		position: absolute;
		left: 0.9rem;
		top: 50%;
		transform: translateY(-50%);
		pointer-events: none;
	}
	.search input {
		padding-left: 2.6rem;
		border-radius: 999px;
	}
	.filter-button {
		width: var(--tap);
		padding: 0;
		position: relative;
	}
	.filter-button.active {
		color: var(--accent);
		background: var(--accent-soft);
	}
	.count {
		position: absolute;
		top: 2px;
		right: 2px;
		min-width: 1.15rem;
		height: 1.15rem;
		padding: 0 0.25rem;
		font-size: 0.7rem;
		line-height: 1.15rem;
		color: var(--accent-text);
		background: var(--accent);
		border-radius: 999px;
	}
	.day {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.day h2 {
		margin-top: 0.5rem;
		padding-left: 0.25rem;
	}
	.list {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.5rem;
		padding: 3rem 1rem;
		text-align: center;
	}
	.empty h2 {
		font-size: 1.15rem;
	}
	.empty p {
		margin: 0;
		max-width: 22rem;
	}
	.empty-icon {
		display: grid;
		place-items: center;
		width: 4rem;
		height: 4rem;
		margin-bottom: 0.5rem;
		border-radius: 50%;
		color: var(--accent);
		background: var(--accent-soft);
	}
	.fab {
		width: 60px;
		height: 60px;
		padding: 0;
		border-radius: 20px;
		box-shadow: var(--shadow-raised);
	}
	.dot {
		width: 0.6rem;
		height: 0.6rem;
		border-radius: 50%;
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
	.sheet-actions {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	.eyebrow {
		margin-bottom: 0.5rem;
	}
</style>

<script lang="ts">
	// Report for a therapist (X-3): choose, preview, then save an unencrypted PDF.
	import FileDown from '@lucide/svelte/icons/file-down';
	import { onMount } from 'svelte';

	import { entriesList, reportPdf, tagsList } from '#lib/api/index.js';
	import type { Entry, EntryKindTag, Tag } from '#lib/api/index.js';
	import { addDays, localToday } from '#lib/chart.js';
	import ReportPreview from '#lib/components/ReportPreview.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { buildReport } from '#lib/report.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';
	import { endOfDay, startOfDay } from '#lib/time.js';

	const KINDS: EntryKindTag[] = ['mood', 'fear', 'event', 'sleep', 'nightmare', 'note'];

	let from = $state(addDays(localToday(), -30));
	let to = $state(localToday());
	let kinds = $state<EntryKindTag[]>([...KINDS]);
	let tagFilter = $state<string[]>([]);
	let tags = $state<Tag[]>([]);
	let entries = $state<Entry[] | null>(null);
	let message = $state('');
	let busy = $state(false);

	let tagMap = $derived(new Map(tags.map((tag) => [tag.id, tag])));
	let doc = $derived(
		entries && entries.length > 0
			? buildReport(entries, { from, to, tags: tagMap, t, locale: locale() })
			: null
	);

	const toggle = <T,>(list: T[], item: T) =>
		list.includes(item) ? list.filter((i) => i !== item) : [...list, item];

	onMount(async () => {
		tags = await tagsList().catch(() => []);
	});

	$effect(() => {
		const filter = {
			kinds: kinds.length > 0 ? [...kinds] : KINDS,
			tags: [...tagFilter],
			from: from ? startOfDay(from) : undefined,
			to: to ? endOfDay(to) : undefined
		};
		const timer = setTimeout(async () => {
			try {
				entries = kinds.length > 0 ? await entriesList(filter) : [];
			} catch (e) {
				if (!(await handleLocked(e))) message = errorText(e);
			}
		}, 150);
		return () => clearTimeout(timer);
	});

	async function save() {
		if (!doc || busy) return;
		busy = true;
		message = '';
		try {
			if (await reportPdf($state.snapshot(doc))) message = t('report.saved');
		} catch (e) {
			if (!(await handleLocked(e))) message = errorText(e);
		} finally {
			busy = false;
		}
	}
</script>

<UnlockedShell title={t('report.title')} back="/settings">
	<div class="stack">
		<p class="muted">{t('report.intro')}</p>

		<div class="card stack">
			<div class="dates">
				<div>
					<label for="from">{t('filter.from')}</label>
					<input id="from" type="date" bind:value={from} max={to} />
				</div>
				<div>
					<label for="to">{t('filter.to')}</label>
					<input id="to" type="date" bind:value={to} min={from} />
				</div>
			</div>
			<div>
				<p class="eyebrow">{t('report.kinds')}</p>
				<div class="chips" role="group" aria-label={t('report.kinds')}>
					{#each KINDS as kind (kind)}
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
					<p class="eyebrow">{t('report.tags')}</p>
					<div class="chips" role="group" aria-label={t('report.tags')}>
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
		</div>

		<h2>{t('report.preview')}</h2>
		{#if entries && !doc}
			<p class="muted">{t('report.empty')}</p>
		{:else if doc}
			<ReportPreview {doc} />
			<p class="notice">{t('report.warning')}</p>
			<button type="button" onclick={save} disabled={busy}>
				<FileDown size={20} />{busy ? t('common.working') : t('report.save')}
			</button>
		{/if}
		{#if message}<p class="notice" aria-live="polite">{message}</p>{/if}
	</div>
</UnlockedShell>

<style>
	.dates {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}
	.eyebrow {
		margin-bottom: 0.5rem;
	}
	.dot {
		width: 0.6rem;
		height: 0.6rem;
		border-radius: 50%;
	}
</style>

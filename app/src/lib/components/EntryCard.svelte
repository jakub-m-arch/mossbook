<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Entry, Tag } from '#lib/api/index.js';
	import { kindTag, preview } from '#lib/entries.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';

	import MoodIcon from './MoodIcon.svelte';

	let { entry, tags }: { entry: Entry; tags: Map<string, Tag> } = $props();

	let when = $derived(
		new Intl.DateTimeFormat(locale(), { dateStyle: 'medium', timeStyle: 'short' }).format(
			new Date(entry.occurred_at)
		)
	);
	let text = $derived(preview(entry));
</script>

<a class="card entry" href={resolve('/entry/[id]', { id: entry.id })}>
	<div class="top">
		{#if entry.mood}<MoodIcon value={entry.mood} size={28} />{/if}
		<strong>{t(`kind.${kindTag(entry.kind)}`)}</strong>
		{#if entry.intensity !== null}<span class="badge">{entry.intensity}/10</span>{/if}
		{#if entry.incomplete}<span class="badge notice-badge">{t('entry.incomplete')}</span>{/if}
		<time class="muted" datetime={entry.occurred_at}>{when}</time>
	</div>
	{#if text}<p>{text}</p>{/if}
	{#if entry.tags.length > 0}
		<div class="tags">
			{#each entry.tags as id (id)}
				{@const tag = tags.get(id)}
				{#if tag}<span class="tag"
						><span class="dot" style:background={tag.color}></span>{tag.name}</span
					>{/if}
			{/each}
		</div>
	{/if}
</a>

<style>
	.entry {
		display: block;
		color: inherit;
		text-decoration: none;
	}
	.top {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
	time {
		margin-left: auto;
		font-size: 0.85rem;
	}
	p {
		margin: 0.5rem 0 0;
		overflow-wrap: anywhere;
	}
	.badge {
		padding: 0.05rem 0.5rem;
		font-size: 0.8rem;
		background: var(--surface-2);
		border-radius: 999px;
	}
	.notice-badge {
		background: var(--notice-soft);
	}
	.tags {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
		margin-top: 0.5rem;
		font-size: 0.85rem;
	}
	.tag {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
	}
	.dot {
		width: 0.6rem;
		height: 0.6rem;
		border-radius: 50%;
	}
</style>

<script lang="ts">
	import Mic from '@lucide/svelte/icons/mic';

	import { resolve } from '$app/paths';
	import type { Entry, Tag } from '#lib/api/index.js';
	import { kindTag, preview } from '#lib/entries.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { formatClock } from '#lib/voice/format.js';

	import KindIcon from './KindIcon.svelte';
	import MoodIcon from './MoodIcon.svelte';

	interface Props {
		entry: Entry;
		tags: Map<string, Tag>;
		/** Long press (touch) or right click: the entry's menu. */
		onmenu?: (entry: Entry) => void;
	}

	let { entry, tags, onmenu }: Props = $props();

	let kind = $derived(kindTag(entry.kind));
	let time = $derived(
		new Intl.DateTimeFormat(locale(), { timeStyle: 'short' }).format(new Date(entry.occurred_at))
	);
	let text = $derived(preview(entry, 160));
	/** E-16: hidden content stays hidden until the user asks for it. */
	let revealed = $state(false);
	let hidden = $derived(entry.sensitive && !revealed);

	/** How long a finger rests on the card before its menu opens. */
	const LONG_PRESS_MS = 450;
	let pressing = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let start = { x: 0, y: 0 };
	/** The menu opened from this press: the click that ends it must not navigate. */
	let opened = false;

	function openMenu() {
		cancelPress();
		opened = true;
		onmenu?.(entry);
	}

	function onpointerdown(e: PointerEvent) {
		opened = false;
		if (!onmenu || e.pointerType === 'mouse' || !e.isPrimary) return;
		start = { x: e.clientX, y: e.clientY };
		pressing = true;
		timer = setTimeout(openMenu, LONG_PRESS_MS);
	}

	function onpointermove(e: PointerEvent) {
		if (timer && Math.hypot(e.clientX - start.x, e.clientY - start.y) > 10) cancelPress();
	}

	function cancelPress() {
		clearTimeout(timer);
		timer = undefined;
		pressing = false;
	}

	function oncontextmenu(e: MouseEvent) {
		if (!onmenu) return;
		// Android also reports a long press as a context menu; open it once.
		e.preventDefault();
		if (!opened) openMenu();
	}

	function onclick(e: MouseEvent) {
		if (opened) {
			e.preventDefault();
			opened = false;
		}
	}
</script>

<div class="wrap">
	<a
		class="row card"
		class:pressing
		href={resolve('/entry/[id]', { id: entry.id })}
		{onpointerdown}
		{onpointermove}
		onpointerup={cancelPress}
		onpointercancel={cancelPress}
		onpointerleave={cancelPress}
		{oncontextmenu}
		{onclick}
	>
		<KindIcon {kind} />
		<div class="body">
			<div class="top">
				<span class="title">{t(`kind.${kind}`)}</span>
				{#if entry.mood}<MoodIcon value={entry.mood} size={20} />{/if}
				{#if entry.intensity !== null}<span class="pill">{entry.intensity}/10</span>{/if}
				<time datetime={entry.occurred_at}>{time}</time>
			</div>
			{#if hidden}
				<p class="text">{t('entry.hidden')}</p>
			{:else if text}
				<p class="text">{text}</p>
			{/if}
			{#if !hidden && (entry.incomplete || entry.tags.length > 0 || entry.voice)}
				<div class="meta">
					{#if entry.incomplete}<span class="pill notice-pill">{t('entry.incomplete')}</span>{/if}
					{#if entry.voice}
						<span class="pill voice"
							><Mic size={14} aria-hidden="true" /><span class="sr-only"
								>{t('voice.length', { time: formatClock(entry.voice.duration_ms) })}</span
							><span aria-hidden="true">{formatClock(entry.voice.duration_ms)}</span></span
						>
					{/if}
					{#each entry.tags as id (id)}
						{@const tag = tags.get(id)}
						{#if tag}<span class="tag"
								><span class="dot" style:background={tag.color}></span>{tag.name}</span
							>{/if}
					{/each}
				</div>
			{/if}
		</div>
	</a>
	{#if entry.sensitive && (text || entry.voice)}
		<button
			type="button"
			class="link reveal"
			aria-expanded={revealed}
			onclick={() => (revealed = !revealed)}
		>
			{revealed ? t('entry.conceal') : t('entry.reveal')}
		</button>
	{/if}
</div>

<style>
	.voice {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		font-variant-numeric: tabular-nums;
	}
	.wrap {
		position: relative;
	}
	.reveal {
		position: absolute;
		right: 0.5rem;
		bottom: 0.4rem;
		font-size: 0.85rem;
	}
	.row {
		display: flex;
		gap: 0.85rem;
		align-items: flex-start;
		color: inherit;
		text-decoration: none;
		transition:
			transform 0.1s,
			box-shadow 0.15s;
	}
	.row:hover {
		box-shadow: var(--shadow-raised);
	}
	.row:active {
		transform: scale(0.99);
	}
	/* A held finger slowly presses the card in, until the menu opens. */
	.row.pressing {
		transform: scale(0.96);
		transition: transform 0.45s ease-out;
	}
	.row {
		-webkit-touch-callout: none;
		-webkit-user-select: none;
		user-select: none;
	}
	@media (prefers-reduced-motion: reduce) {
		.row.pressing {
			transform: none;
			box-shadow: var(--shadow-raised);
		}
	}
	.body {
		flex: 1;
		min-width: 0;
	}
	.top {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		min-height: 40px;
	}
	.title {
		font-weight: 650;
	}
	time {
		margin-left: auto;
		font-size: 0.85rem;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.text {
		margin: 0;
		color: var(--text-muted);
		overflow-wrap: anywhere;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.5rem;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.pill {
		padding: 0.1rem 0.55rem;
		font-size: 0.78rem;
		font-weight: 600;
		background: var(--surface-2);
		border-radius: 999px;
	}
	.notice-pill {
		color: var(--notice);
		background: var(--notice-soft);
	}
	.tag {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
	}
	.dot {
		width: 0.55rem;
		height: 0.55rem;
		border-radius: 50%;
	}
</style>

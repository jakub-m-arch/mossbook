<script lang="ts">
	// Frame of every unlocked screen: top bar with the lock button (V-4),
	// bottom tab bar on the main screens, back button on sub-screens.
	import ChartLine from '@lucide/svelte/icons/chart-line';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import LifeBuoy from '@lucide/svelte/icons/life-buoy';
	import Lock from '@lucide/svelte/icons/lock';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import Settings from '@lucide/svelte/icons/settings';
	import { onMount, type Snippet } from 'svelte';

	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { settingsGet, vaultLock, vaultStatus } from '#lib/api/index.js';
	import { detectLocale, setLocale, t } from '#lib/i18n/index.svelte.js';
	import { toLockScreen } from '#lib/session.svelte.js';
	import { flushUnsaved } from '#lib/unsaved.js';

	interface Props {
		title: string;
		/** Sub-screen: a back button to this screen instead of the tab bar. */
		back?: '/timeline' | '/settings' | '/session';
		/** Floating action (e.g. quick entry) above the tab bar. */
		fab?: Snippet;
		children: Snippet;
	}

	let { title, back, fab, children }: Props = $props();
	let unreadable = $state(0);

	const TABS = [
		{ href: '/timeline', label: 'nav.journal', icon: NotebookPen },
		{ href: '/plan', label: 'nav.plan', icon: LifeBuoy },
		{ href: '/patterns', label: 'nav.patterns', icon: ChartLine },
		{ href: '/settings', label: 'nav.settings', icon: Settings }
	] as const;

	onMount(() => {
		vaultStatus()
			.then((status) => {
				if (status.state !== 'unlocked') return toLockScreen();
				unreadable = status.unreadable_records;
				// The saved language applies once settings can be read (D-2).
				settingsGet()
					.then((settings) => setLocale(settings.locale ?? detectLocale()))
					.catch(() => {});
			})
			.catch(() => toLockScreen());
	});

	async function lock() {
		// An open entry form is kept first, so locking in a hurry loses nothing (D-20).
		await flushUnsaved();
		// The `vault-locked` event also navigates; going there directly avoids a flash.
		await vaultLock().catch(() => {});
		await toLockScreen();
	}
</script>

<div class="shell" class:with-tabs={!back}>
	<header>
		{#if back}
			<a class="button icon" href={resolve(back)} aria-label={t('common.back')}>
				<ChevronLeft size={24} />
			</a>
		{/if}
		<h1>{title}</h1>
		<!-- V-4: one tap, always visible while unlocked. -->
		<button
			type="button"
			class="icon"
			onclick={lock}
			aria-label={t('common.lock')}
			title={t('common.lock')}
		>
			<Lock size={22} />
		</button>
	</header>

	<main>
		{#if unreadable > 0}
			<p class="notice" role="status">{t('timeline.unreadable', { count: unreadable })}</p>
		{/if}
		{@render children()}
	</main>

	{#if fab}<div class="fab">{@render fab()}</div>{/if}

	{#if !back}
		<nav aria-label={t('nav.label')}>
			{#each TABS as tab (tab.href)}
				{@const active = page.url.pathname.startsWith(tab.href)}
				<a href={resolve(tab.href)} class:active aria-current={active ? 'page' : undefined}>
					<tab.icon size={22} strokeWidth={active ? 2.3 : 1.8} />
					<span>{t(tab.label)}</span>
				</a>
			{/each}
		</nav>
	{/if}
</div>

<style>
	.shell.with-tabs {
		padding-bottom: calc(var(--bar) + env(safe-area-inset-bottom));
	}
	header {
		position: sticky;
		top: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		gap: 0.25rem;
		max-width: 40rem;
		margin: 0 auto;
		padding: calc(0.5rem + env(safe-area-inset-top)) 0.6rem 0.5rem 1rem;
		background: color-mix(in srgb, var(--bg) 88%, transparent);
		backdrop-filter: blur(12px);
	}
	header:has(.icon:first-child) {
		padding-left: 0.4rem;
	}
	header h1 {
		flex: 1;
		margin: 0;
		font-size: 1.35rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	main {
		padding-top: 0.5rem;
	}
	.fab {
		position: fixed;
		right: max(1rem, calc((100vw - 40rem) / 2 + 1rem));
		bottom: calc(var(--bar) + 1rem + env(safe-area-inset-bottom));
		z-index: 20;
	}
	nav {
		position: fixed;
		inset: auto 0 0 0;
		z-index: 20;
		display: flex;
		justify-content: center;
		padding-bottom: env(safe-area-inset-bottom);
		background: color-mix(in srgb, var(--surface) 92%, transparent);
		backdrop-filter: blur(12px);
		border-top: 1px solid var(--border);
	}
	nav a {
		flex: 1;
		max-width: 10rem;
		height: var(--bar);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.15rem;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--text-muted);
		text-decoration: none;
	}
	nav a.active {
		color: var(--accent);
	}
</style>

<script lang="ts">
	import { onMount, type Snippet } from 'svelte';

	import { vaultLock, vaultStatus } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { toLockScreen } from '#lib/session.svelte.js';

	let { title, children }: { title: string; children: Snippet } = $props();
	let unreadable = $state(0);

	onMount(() => {
		vaultStatus()
			.then((status) => {
				if (status.state !== 'unlocked') return toLockScreen();
				unreadable = status.unreadable_records;
			})
			.catch(() => toLockScreen());
	});

	async function lock() {
		// The `vault-locked` event also navigates; going there directly avoids a flash.
		await vaultLock().catch(() => {});
		await toLockScreen();
	}
</script>

<header>
	<h1>{title}</h1>
	<!-- V-4: one tap, always visible while unlocked. -->
	<button type="button" class="secondary" onclick={lock}>{t('common.lock')}</button>
</header>

<main>
	{#if unreadable > 0}
		<p class="notice" role="status">{t('timeline.unreadable', { count: unreadable })}</p>
	{/if}
	{@render children()}
</main>

<style>
	header {
		position: sticky;
		top: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		max-width: 36rem;
		margin: 0 auto;
		padding: 0.5rem 1rem;
		background: var(--bg);
	}
	header h1 {
		margin: 0;
		font-size: 1.25rem;
	}
</style>

<script lang="ts">
	import { onMount } from 'svelte';

	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import { t } from '#lib/i18n/index.svelte.js';
	import { curtainCovered, installSessionHooks } from '#lib/session.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	onMount(installSessionHooks);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Mossbook</title>
</svelte:head>

{@render children()}

{#if curtainCovered()}
	<!-- Desktop focus loss: hide the content without locking (decision D-11). -->
	<div class="curtain" aria-hidden="true">{t('curtain.text')}</div>
{/if}

<style>
	.curtain {
		position: fixed;
		inset: 0;
		z-index: 1000;
		display: grid;
		place-items: center;
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--text-muted);
		background: var(--bg);
	}
</style>

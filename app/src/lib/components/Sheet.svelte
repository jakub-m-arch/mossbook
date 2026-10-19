<script lang="ts">
	// Bottom sheet on phones, centered dialog on wide screens. Built on <dialog>
	// for focus trapping, Escape and the backdrop.
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';

	import { t } from '#lib/i18n/index.svelte.js';

	interface Props {
		title: string;
		onclose?: () => void;
		children: Snippet;
	}

	let { title, onclose, children }: Props = $props();
	let dialog: HTMLDialogElement;
	const id = `sheet-${Math.random().toString(36).slice(2)}`;

	export function open() {
		dialog.showModal();
	}

	export function close() {
		dialog.close();
	}
</script>

<dialog
	bind:this={dialog}
	aria-labelledby={id}
	{onclose}
	onclick={(e) => {
		if (e.target === dialog) dialog.close();
	}}
>
	<div class="sheet">
		<div class="grabber" aria-hidden="true"></div>
		<header>
			<h2 {id}>{title}</h2>
			<button
				type="button"
				class="icon"
				onclick={() => dialog.close()}
				aria-label={t('common.close')}
			>
				<X size={22} />
			</button>
		</header>
		{@render children()}
	</div>
</dialog>

<style>
	dialog {
		width: 100%;
		max-width: 34rem;
		max-height: 92vh;
		margin: auto auto 0;
		padding: 0;
		color: var(--text);
		background: var(--surface);
		border: 0;
		border-radius: var(--radius-lg) var(--radius-lg) 0 0;
		box-shadow: var(--shadow-raised);
	}
	dialog[open] {
		animation: rise 0.22s ease-out;
	}
	dialog::backdrop {
		background: rgb(10 12 10 / 0.35);
		backdrop-filter: blur(2px);
	}
	.sheet {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 0.5rem 1.25rem calc(1.25rem + env(safe-area-inset-bottom));
	}
	.grabber {
		width: 2.5rem;
		height: 4px;
		margin: 0 auto;
		border-radius: 2px;
		background: var(--surface-3);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
	}
	h2 {
		font-size: 1.2rem;
	}
	@media (min-width: 40rem) {
		dialog {
			margin: auto;
			border-radius: var(--radius-lg);
		}
		.grabber {
			display: none;
		}
	}
	@keyframes rise {
		from {
			transform: translateY(24px);
			opacity: 0;
		}
	}
</style>

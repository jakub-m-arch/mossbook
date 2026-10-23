<script lang="ts">
	// A button whose action is confirmed in a modal sheet (Sheet: bottom sheet on
	// phones, centered dialog on wide screens); native confirm() is unreliable in
	// mobile webviews. The question's first sentence (up to "?") is the title,
	// the rest explains. Focus starts on Cancel, so Enter never confirms by accident.
	import { t } from '#lib/i18n/index.svelte.js';

	import Sheet from './Sheet.svelte';

	interface Props {
		label: string;
		question: string;
		onconfirm: () => void | Promise<void>;
		/** `false`: no button of its own; the caller opens the sheet with `ask()`. */
		button?: boolean;
	}

	let { label, question, onconfirm, button = true }: Props = $props();
	let sheet: Sheet;
	let cancel: HTMLButtonElement;
	let busy = $state(false);

	let parts = $derived(question.match(/^(.*?\?)\s*(.*)$/s));
	let title = $derived(parts ? parts[1] : question);
	let detail = $derived(parts ? parts[2] : '');

	/** Opens the sheet; also for callers that start it from elsewhere (a menu). */
	export function ask() {
		sheet.open();
		cancel.focus();
	}

	async function confirm() {
		busy = true;
		try {
			await onconfirm();
		} finally {
			busy = false;
			// The action may have removed this component (and the sheet) already.
			sheet?.close();
		}
	}
</script>

{#if button}
	<button type="button" class="secondary" onclick={ask}>{label}</button>
{/if}

<Sheet bind:this={sheet} {title}>
	{#if detail}<p>{detail}</p>{/if}
	<div class="row">
		<button
			type="button"
			class="secondary"
			bind:this={cancel}
			disabled={busy}
			onclick={() => sheet.close()}
		>
			{t('common.cancel')}
		</button>
		<button type="button" disabled={busy} onclick={confirm}>
			{busy ? t('common.working') : label}
		</button>
	</div>
</Sheet>

<style>
	.row {
		display: flex;
		gap: 0.5rem;
	}
	.row button {
		flex: 1;
	}
	p {
		margin: 0;
	}
</style>

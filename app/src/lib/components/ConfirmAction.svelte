<script lang="ts">
	// Inline two-step confirmation; native confirm() is unreliable in mobile webviews.
	import { t } from '#lib/i18n/index.svelte.js';

	interface Props {
		label: string;
		question: string;
		onconfirm: () => void | Promise<void>;
	}

	let { label, question, onconfirm }: Props = $props();
	let asking = $state(false);
</script>

{#if asking}
	<div class="card stack" role="group" aria-label={question}>
		<p>{question}</p>
		<div class="row">
			<button
				type="button"
				onclick={async () => {
					await onconfirm();
					asking = false;
				}}>{label}</button
			>
			<button type="button" class="secondary" onclick={() => (asking = false)}>
				{t('common.cancel')}
			</button>
		</div>
	</div>
{:else}
	<button type="button" class="secondary" onclick={() => (asking = true)}>{label}</button>
{/if}

<style>
	.row {
		display: flex;
		gap: 0.5rem;
		flex-wrap: wrap;
	}
	p {
		margin: 0;
	}
</style>

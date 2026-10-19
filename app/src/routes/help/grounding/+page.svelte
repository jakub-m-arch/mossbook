<script lang="ts">
	import { GROUNDING_STEPS as STEPS } from '#lib/exercises.js';
	import { t } from '#lib/i18n/index.svelte.js';

	let step = $state(0);
</script>

<main class="stack">
	<button type="button" class="secondary back" onclick={() => history.back()}>
		← {t('common.back')}
	</button>
	<h1>{t('exercise.grounding')}</h1>
	<p>{t('exercise.introGrounding')}</p>

	<div class="card stack step" aria-live="polite">
		{#if step < STEPS.length}
			<p class="muted">{t('exercise.step', { n: step + 1 })}</p>
			<p class="prompt">{t(`exercise.${STEPS[step]}`)}</p>
			<button type="button" onclick={() => (step += 1)}>{t('exercise.next')}</button>
		{:else}
			<p class="prompt">{t('exercise.done')}</p>
			<button type="button" class="secondary" onclick={() => (step = 0)}
				>{t('exercise.again')}</button
			>
		{/if}
	</div>
</main>

<style>
	.back {
		align-self: flex-start;
	}
	.step {
		min-height: 14rem;
		justify-content: space-between;
	}
	.step p {
		margin: 0;
	}
	.prompt {
		font-size: 1.4rem;
		font-weight: 600;
		line-height: 1.35;
	}
</style>

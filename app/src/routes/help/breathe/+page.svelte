<script lang="ts">
	import { page } from '$app/state';
	import Breathing from '#lib/components/Breathing.svelte';
	import { BREATHING } from '#lib/exercises.js';
	import { t } from '#lib/i18n/index.svelte.js';

	const requested = page.url.searchParams.get('pattern');
	const pattern = requested === 'box' ? BREATHING.box : BREATHING['478'];
</script>

<main class="stack">
	<button type="button" class="secondary back" onclick={() => history.back()}>
		← {t('common.back')}
	</button>
	<h1>{t(pattern.title)}</h1>
	<p>{t(pattern.intro)}</p>
	<Breathing phases={pattern.phases} />
</main>

<style>
	.back {
		align-self: flex-start;
	}
</style>

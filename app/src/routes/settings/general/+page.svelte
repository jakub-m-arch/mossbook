<script lang="ts">
	import { onMount } from 'svelte';

	import { settingsGet, settingsSave } from '#lib/api/index.js';
	import type { AutoLock, Settings } from '#lib/api/index.js';
	import FingerprintUnlock from '#lib/components/FingerprintUnlock.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { detectLocale, setLocale, t } from '#lib/i18n/index.svelte.js';
	import { createMessages } from '#lib/messages.svelte.js';

	const AUTOLOCK: AutoLock[] = ['immediately', 'after1_min', 'after5_min', 'after15_min'];
	/** Each language in its own name, so it can be found whatever is shown now. */
	const LANGUAGES: Array<{ value: Settings['locale']; name: string | null }> = [
		{ value: null, name: null },
		{ value: 'pl', name: 'Polski' },
		{ value: 'en', name: 'English' },
		{ value: 'uk', name: 'Українська' }
	];

	const { messages, attempt } = createMessages();
	let settings = $state<Settings | null>(null);

	onMount(() => {
		void attempt('settings', async () => {
			settings = await settingsGet();
		});
	});

	function saveAutolock(value: AutoLock) {
		if (!settings) return;
		const next = { ...$state.snapshot(settings), autolock: value } as Settings;
		void attempt('settings', async () => {
			settings = await settingsSave(next);
			return t('settings.saved');
		});
	}

	function saveLanguage(value: Settings['locale']) {
		if (!settings) return;
		const next = { ...$state.snapshot(settings), locale: value } as Settings;
		setLocale(value ?? detectLocale());
		void attempt('language', async () => {
			settings = await settingsSave(next);
		});
	}
</script>

<UnlockedShell title={t('settings.general')} back="/settings">
	<div class="stack">
		<section class="card stack" aria-labelledby="language">
			<h2 id="language">{t('settings.language')}</h2>
			{#if settings}
				<fieldset class="choices">
					<legend class="sr-only">{t('settings.language')}</legend>
					{#each LANGUAGES as option (option.value ?? 'system')}
						<label>
							<input
								type="radio"
								name="language"
								checked={settings.locale === option.value}
								onchange={() => saveLanguage(option.value)}
							/>
							{option.name ?? t('settings.systemLanguage')}
						</label>
					{/each}
				</fieldset>
			{/if}
			{#if messages.language}<p class="notice" aria-live="polite">{messages.language}</p>{/if}
		</section>

		<section class="card stack" aria-labelledby="autolock">
			<h2 id="autolock">{t('settings.autolock')}</h2>
			{#if settings}
				<fieldset class="choices">
					<legend class="sr-only">{t('settings.autolock')}</legend>
					{#each AUTOLOCK as option (option)}
						<label>
							<input
								type="radio"
								name="autolock"
								value={option}
								checked={settings.autolock === option}
								onchange={() => saveAutolock(option)}
							/>
							{t(`settings.${option}`)}
						</label>
					{/each}
				</fieldset>
			{/if}
			{#if messages.settings}<p class="muted" aria-live="polite">{messages.settings}</p>{/if}
		</section>

		<FingerprintUnlock />
	</div>
</UnlockedShell>

<style>
	h2 {
		margin: 0;
		font-size: 1.15rem;
	}
	.choices {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		border: 0;
		margin: 0;
		padding: 0;
	}
	.choices label {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		min-height: var(--tap);
		font-weight: 500;
	}
	.choices input {
		width: 1.3rem;
		min-height: 1.3rem;
		flex: none;
	}
</style>

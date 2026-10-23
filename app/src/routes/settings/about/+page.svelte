<script lang="ts">
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import { onMount } from 'svelte';

	import { appInfo, openReleases } from '#lib/api/index.js';
	import type { AppInfo } from '#lib/api/index.js';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { createMessages } from '#lib/messages.svelte.js';

	const { messages, attempt } = createMessages();
	let info = $state<AppInfo | null>(null);
	/** Offline nudge: no network, just the age of this build (D-15). */
	const OLD_AFTER_DAYS = 90;
	let isOld = $derived(!!info && Date.now() / 1000 - info.built_at > OLD_AFTER_DAYS * 86_400);
	/** Licence texts (~0.8 MB), loaded only when the section is opened. */
	let licenses = $state<string | null>(null);

	async function loadLicenses(event: Event) {
		if ((event.currentTarget as HTMLDetailsElement).open && licenses === null) {
			licenses = (await import('#lib/assets/licenses.txt?raw')).default;
		}
	}

	onMount(() => {
		appInfo()
			.then((i) => (info = i))
			.catch(() => {});
	});
</script>

<UnlockedShell title={t('settings.about')} back="/settings">
	<section class="card stack" aria-labelledby="about">
		<h2 id="about">{t('about.title')}</h2>
		{#if info}
			<p class="muted">
				{t('about.version', {
					version: info.version,
					date: new Intl.DateTimeFormat(locale(), { dateStyle: 'long' }).format(
						new Date(info.built_at * 1000)
					)
				})}
			</p>
			{#if isOld}<p class="notice">{t('about.old')}</p>{/if}
		{/if}
		<button
			type="button"
			class="secondary"
			onclick={() => attempt('about', async () => void (await openReleases()))}
		>
			<ExternalLink size={18} />{t('about.check')}
		</button>
		<p class="muted small">{t('about.checkHint')} {t('about.watch')}</p>
		{#if messages.about}<p class="notice" aria-live="polite">{messages.about}</p>{/if}
		<p class="muted small">{t('about.legal')}</p>
		<details ontoggle={loadLicenses}>
			<summary>{t('about.licenses')}</summary>
			{#if licenses}<pre class="licenses">{licenses}</pre>{/if}
		</details>
	</section>
</UnlockedShell>

<style>
	h2 {
		margin: 0;
		font-size: 1.15rem;
	}
	summary {
		min-height: var(--tap);
		display: flex;
		align-items: center;
		color: var(--accent);
		font-weight: 600;
		cursor: pointer;
	}
	.licenses {
		max-height: 60vh;
		overflow: auto;
		margin: 0;
		font-size: 0.75rem;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>

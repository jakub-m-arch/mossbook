<script lang="ts">
	// Fingerprint unlock (V-7): shown where the device has strong biometrics, or
	// while it is still on (so it can be turned off after removing fingerprints).
	// A strong sensor without fingerprints gets a hint where to add one.
	import { onMount } from 'svelte';

	import { deviceUnlockDisable, deviceUnlockEnable, deviceUnlockStatus } from '#lib/api/index.js';
	import type { DeviceUnlockStatus } from '#lib/api/index.js';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	let status = $state<DeviceUnlockStatus | null>(null);
	let password = $state('');
	let busy = $state(false);
	let message = $state('');

	onMount(() => {
		deviceUnlockStatus()
			.then((s) => (status = s))
			.catch(() => {});
	});

	async function run(action: () => Promise<string>) {
		busy = true;
		message = '';
		try {
			message = await action();
		} catch (e) {
			if (!(await handleLocked(e))) message = errorText(e);
		} finally {
			busy = false;
		}
	}

	function enable(event: SubmitEvent) {
		event.preventDefault();
		if (busy || password.length === 0) return;
		void run(async () => {
			const on = await deviceUnlockEnable(password, {
				title: t('fingerprint.promptTitle'),
				subtitle: null,
				cancel: t('common.cancel')
			});
			if (!on) return '';
			password = '';
			status = { available: true, enrollable: false, enabled: true };
			return t('fingerprint.enabled');
		});
	}

	function disable() {
		void run(async () => {
			await deviceUnlockDisable();
			status = { available: status?.available ?? false, enrollable: false, enabled: false };
			return t('fingerprint.disabled');
		});
	}
</script>

{#if status?.enrollable && !status.enabled}
	<section class="card stack" aria-labelledby="fingerprint">
		<h2 id="fingerprint">{t('fingerprint.title')}</h2>
		<p class="muted">{t('fingerprint.enrollHint')}</p>
	</section>
{:else if status && (status.available || status.enabled)}
	<section class="card stack" aria-labelledby="fingerprint">
		<h2 id="fingerprint">{t('fingerprint.title')}</h2>
		{#if status.enabled}
			<p>{t('fingerprint.on')}</p>
			<button type="button" class="secondary" onclick={disable} disabled={busy}>
				{t('fingerprint.turnOff')}
			</button>
		{:else}
			<p class="muted">{t('fingerprint.intro')}</p>
			<form class="stack" onsubmit={enable}>
				<PasswordField
					id="fingerprint-password"
					label={t('fingerprint.password')}
					bind:value={password}
					autocomplete="current-password"
				/>
				<button type="submit" disabled={busy || password.length === 0}>
					{busy ? t('common.working') : t('fingerprint.turnOn')}
				</button>
			</form>
		{/if}
		<p class="notice">{t('fingerprint.reminder')}</p>
		{#if message}<p class="muted" aria-live="polite">{message}</p>{/if}
	</section>
{/if}

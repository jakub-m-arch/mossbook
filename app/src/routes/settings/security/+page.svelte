<script lang="ts">
	import { onMount } from 'svelte';

	import {
		isAppError,
		settingsGet,
		vaultChangePassword,
		vaultVerifyRecovery
	} from '#lib/api/index.js';
	import type { Settings } from '#lib/api/index.js';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import StrengthMeter from '#lib/components/StrengthMeter.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { createMessages } from '#lib/messages.svelte.js';
	import { MIN_PASSWORD_CHARS } from '#lib/password.js';

	const { messages, attempt } = createMessages();
	let settings = $state<Settings | null>(null);
	let oldPassword = $state('');
	let newPassword = $state('');
	let repeat = $state('');
	let recoveryPhrase = $state('');

	let passwordReady = $derived(
		oldPassword.length > 0 &&
			[...newPassword].length >= MIN_PASSWORD_CHARS &&
			newPassword === repeat
	);

	onMount(() => {
		void attempt('password', async () => {
			settings = await settingsGet();
		});
	});

	function changePassword(e: SubmitEvent) {
		e.preventDefault();
		if (!passwordReady) return;
		void attempt('password', async () => {
			await vaultChangePassword(oldPassword, newPassword);
			oldPassword = newPassword = repeat = '';
			const auto = settings?.auto_backup ? ` ${t('password.autoReplaced')}` : '';
			return `${t('password.changed')} ${t('password.oldBackups')}${auto}`;
		});
	}

	function checkRecovery(e: SubmitEvent) {
		e.preventDefault();
		if (!recoveryPhrase.trim()) return;
		void attempt('recovery', async () => {
			try {
				await vaultVerifyRecovery(recoveryPhrase);
			} catch (err) {
				if (isAppError(err) && err.code === 'wrong_recovery_key') return t('recoveryCheck.wrong');
				throw err;
			}
			recoveryPhrase = '';
			return t('recoveryCheck.ok');
		});
	}
</script>

<UnlockedShell title={t('settings.security')} back="/settings">
	<div class="stack">
		<section class="card stack" aria-labelledby="password">
			<h2 id="password">{t('password.title')}</h2>
			<form class="stack" onsubmit={changePassword}>
				<PasswordField
					id="old-password"
					label={t('password.current')}
					bind:value={oldPassword}
					autocomplete="current-password"
				/>
				<div>
					<PasswordField
						id="new-password"
						label={t('password.new')}
						bind:value={newPassword}
						autocomplete="new-password"
						describedby="new-strength"
					/>
					<StrengthMeter id="new-strength" password={newPassword} />
				</div>
				<PasswordField
					id="repeat-password"
					label={t('common.passwordRepeat')}
					bind:value={repeat}
					autocomplete="new-password"
				/>
				{#if repeat && newPassword !== repeat}<p class="notice">
						{t('setup.password.mismatch')}
					</p>{/if}
				<button type="submit" disabled={!passwordReady}>{t('password.submit')}</button>
				{#if messages.password}<p class="notice" aria-live="polite">{messages.password}</p>{/if}
			</form>
		</section>

		<section class="card stack" aria-labelledby="recovery-check">
			<h2 id="recovery-check">{t('recoveryCheck.title')}</h2>
			<p class="muted">{t('recoveryCheck.intro')}</p>
			<form class="stack" onsubmit={checkRecovery}>
				<label for="recovery-phrase">{t('recoveryCheck.label')}</label>
				<textarea
					id="recovery-phrase"
					rows="4"
					bind:value={recoveryPhrase}
					autocomplete="off"
					autocapitalize="off"
					spellcheck="false"></textarea>
				<button type="submit" class="secondary" disabled={!recoveryPhrase.trim()}>
					{t('recoveryCheck.submit')}
				</button>
				{#if messages.recovery}<p class="notice" aria-live="polite">{messages.recovery}</p>{/if}
			</form>
		</section>
	</div>
</UnlockedShell>

<style>
	h2 {
		margin: 0;
		font-size: 1.15rem;
	}
</style>

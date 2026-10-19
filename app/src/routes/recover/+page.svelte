<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { vaultUnlockRecovery } from '#lib/api/index.js';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import StrengthMeter from '#lib/components/StrengthMeter.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { MIN_PASSWORD_CHARS } from '#lib/password.js';
	import { errorText } from '#lib/session.svelte.js';

	let phrase = $state('');
	let password = $state('');
	let repeat = $state('');
	let error = $state('');
	let busy = $state(false);

	let mismatch = $derived(repeat.length > 0 && password !== repeat);
	let ready = $derived(
		phrase.trim().split(/\s+/).length === 24 &&
			[...password].length >= MIN_PASSWORD_CHARS &&
			password === repeat
	);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!ready || busy) return;
		busy = true;
		error = '';
		try {
			await vaultUnlockRecovery(phrase, password);
			phrase = password = repeat = '';
			await goto(resolve('/timeline'), { replaceState: true });
		} catch (e) {
			error = errorText(e);
		} finally {
			busy = false;
		}
	}
</script>

<main class="stack">
	<h1>{t('recover.title')}</h1>
	<p>{t('recover.body')}</p>
	<form class="stack" onsubmit={submit}>
		<div>
			<label for="phrase">{t('recover.phrase')}</label>
			<textarea
				id="phrase"
				bind:value={phrase}
				autocomplete="off"
				autocapitalize="off"
				spellcheck="false"></textarea>
		</div>
		<div>
			<PasswordField
				id="new-password"
				label={t('recover.newPassword')}
				bind:value={password}
				autocomplete="new-password"
				describedby="strength"
			/>
			<StrengthMeter id="strength" {password} />
		</div>
		<PasswordField
			id="repeat"
			label={t('common.passwordRepeat')}
			bind:value={repeat}
			autocomplete="new-password"
		/>
		<div aria-live="polite">
			{#if mismatch}<p class="notice">{t('setup.password.mismatch')}</p>{/if}
			{#if error}<p class="notice">{error}</p>{/if}
		</div>
		<button type="submit" disabled={!ready || busy}>
			{busy ? t('common.working') : t('recover.submit')}
		</button>
	</form>
	<a class="button secondary" href={resolve('/lock')}>{t('common.back')}</a>
</main>

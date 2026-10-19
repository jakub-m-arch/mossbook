<script lang="ts">
	import { onMount } from 'svelte';

	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		vaultCancelCreate,
		vaultConfirmRecovery,
		vaultCreate,
		vaultStatus
	} from '#lib/api/index.js';
	import type { RecoverySetup } from '#lib/api/index.js';
	import Logo from '#lib/components/Logo.svelte';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import StrengthMeter from '#lib/components/StrengthMeter.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { MIN_PASSWORD_CHARS } from '#lib/password.js';
	import { errorText } from '#lib/session.svelte.js';

	// V-1: welcome → password → recovery phrase → retype 3 words. Nothing is
	// written to disk until the words match (decision D-3).
	type Step = 'welcome' | 'password' | 'phrase' | 'confirm';

	let step = $state<Step>('welcome');
	let password = $state('');
	let repeat = $state('');
	let setup = $state<RecoverySetup | null>(null);
	let saved = $state(false);
	let answers = $state<string[]>(['', '', '']);
	let error = $state('');
	let busy = $state(false);

	let mismatch = $derived(repeat.length > 0 && password !== repeat);
	let passwordReady = $derived([...password].length >= MIN_PASSWORD_CHARS && password === repeat);
	let answersReady = $derived(answers.every((a) => a.trim().length > 0));

	onMount(async () => {
		const status = await vaultStatus().catch(() => null);
		if (status?.state === 'locked') await goto(resolve('/lock'), { replace: true });
		if (status?.state === 'unlocked') await goto(resolve('/timeline'), { replace: true });
	});

	async function createVault(event: SubmitEvent) {
		event.preventDefault();
		if (!passwordReady || busy) return;
		busy = true;
		error = '';
		try {
			// Calibrates Argon2id on this device (about a second).
			setup = await vaultCreate(password);
			password = repeat = '';
			saved = false;
			answers = ['', '', ''];
			step = 'phrase';
		} catch (e) {
			error = errorText(e);
		} finally {
			busy = false;
		}
	}

	async function backToPassword() {
		await vaultCancelCreate().catch(() => {});
		setup = null;
		step = 'password';
	}

	async function confirm(event: SubmitEvent) {
		event.preventDefault();
		if (!setup || !answersReady || busy) return;
		busy = true;
		error = '';
		try {
			await vaultConfirmRecovery(
				setup.positions.map((position, i) => ({ position, word: answers[i] }))
			);
			setup = null;
			answers = ['', '', ''];
			await goto(resolve('/timeline'), { replace: true });
		} catch (e) {
			error = errorText(e);
		} finally {
			busy = false;
		}
	}
</script>

<main class="auth">
	{#if step === 'welcome'}
		<div class="brand">
			<Logo size={72} />
			<h1>{t('setup.welcome.title')}</h1>
			<p class="muted">{t('setup.welcome.body')}</p>
		</div>
		<p class="notice">{t('setup.welcome.disclaimer')}</p>
		<button type="button" onclick={() => (step = 'password')}>{t('setup.welcome.start')}</button>
		<a class="button secondary" href={resolve('/restore')}>{t('backup.haveBackup')}</a>
	{:else if step === 'password'}
		<p class="eyebrow">{t('setup.step', { n: 1, total: 3 })}</p>
		<h1>{t('setup.password.title')}</h1>
		<p>{t('setup.password.body')}</p>
		<form class="stack" onsubmit={createVault}>
			<div>
				<PasswordField
					id="password"
					label={t('common.password')}
					bind:value={password}
					autocomplete="new-password"
					autofocus
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
			<button type="submit" disabled={!passwordReady || busy}>
				{busy ? t('common.working') : t('common.next')}
			</button>
		</form>
		<button type="button" class="link" onclick={() => (step = 'welcome')}>
			{t('common.back')}
		</button>
	{:else if step === 'phrase' && setup}
		<p class="eyebrow">{t('setup.step', { n: 2, total: 3 })}</p>
		<h1>{t('setup.phrase.title')}</h1>
		<p>{t('setup.phrase.body')}</p>
		<p class="notice">{t('setup.phrase.warning')}</p>
		<ol class="words card">
			{#each setup.words as word, i (i)}
				<li><span class="muted">{i + 1}.</span><span>{word}</span></li>
			{/each}
		</ol>
		<label class="check">
			<input type="checkbox" bind:checked={saved} />
			<span>{t('setup.phrase.saved')}</span>
		</label>
		<button type="button" disabled={!saved} onclick={() => (step = 'confirm')}>
			{t('common.next')}
		</button>
		<button type="button" class="link" onclick={backToPassword}>{t('common.back')}</button>
	{:else if step === 'confirm' && setup}
		<p class="eyebrow">{t('setup.step', { n: 3, total: 3 })}</p>
		<h1>{t('setup.confirm.title')}</h1>
		<p>{t('setup.confirm.body')}</p>
		<form class="stack" onsubmit={confirm}>
			{#each setup.positions as position, i (position)}
				<div>
					<label for="word-{i}">{t('setup.confirm.word', { n: position + 1 })}</label>
					<!-- svelte-ignore a11y_autofocus -->
					<input
						id="word-{i}"
						bind:value={answers[i]}
						autocomplete="off"
						autocapitalize="off"
						spellcheck="false"
						autofocus={i === 0}
					/>
				</div>
			{/each}
			<div aria-live="polite">
				{#if error}<p class="notice">{error}</p>{/if}
			</div>
			<button type="submit" disabled={!answersReady || busy}>
				{busy ? t('common.working') : t('setup.confirm.submit')}
			</button>
		</form>
		<button type="button" class="link" onclick={() => (step = 'phrase')}>
			{t('common.back')}
		</button>
	{/if}
</main>

<style>
	main :global(h1) {
		margin: 0;
	}
	.eyebrow {
		margin-bottom: -0.5rem;
	}
	.words {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
		gap: 0.4rem 1rem;
		margin: 0;
		padding: 1rem;
		list-style: none;
		background: var(--surface);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		font-family: ui-monospace, 'SF Mono', Menlo, monospace;
		font-size: 1.05rem;
	}
	.words li {
		display: grid;
		grid-template-columns: 2.2em 1fr;
	}
	.words li span:first-child {
		text-align: right;
		padding-right: 0.5em;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-height: var(--tap);
		font-weight: 500;
	}
	.check input {
		width: 1.5rem;
		min-height: 1.5rem;
		flex: none;
	}
</style>

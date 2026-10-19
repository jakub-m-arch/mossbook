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
		if (status?.state === 'locked') await goto(resolve('/lock'), { replaceState: true });
		if (status?.state === 'unlocked') await goto(resolve('/timeline'), { replaceState: true });
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
			await goto(resolve('/timeline'), { replaceState: true });
		} catch (e) {
			error = errorText(e);
		} finally {
			busy = false;
		}
	}
</script>

<main class="stack">
	{#if step === 'welcome'}
		<h1>{t('setup.welcome.title')}</h1>
		<p>{t('setup.welcome.body')}</p>
		<p class="notice">{t('setup.welcome.disclaimer')}</p>
		<button type="button" onclick={() => (step = 'password')}>{t('setup.welcome.start')}</button>
	{:else if step === 'password'}
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
		<button type="button" class="secondary" onclick={() => (step = 'welcome')}>
			{t('common.back')}
		</button>
	{:else if step === 'phrase' && setup}
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
		<button type="button" class="secondary" onclick={backToPassword}>{t('common.back')}</button>
	{:else if step === 'confirm' && setup}
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
		<button type="button" class="secondary" onclick={() => (step = 'phrase')}>
			{t('common.back')}
		</button>
	{/if}
</main>

<style>
	.words {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
		gap: 0.4rem 1rem;
		margin: 0;
		padding: 1rem;
		list-style: none;
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

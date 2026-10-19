<script lang="ts">
	import LifeBuoy from '@lucide/svelte/icons/life-buoy';
	import { onDestroy, onMount } from 'svelte';

	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { isAppError, vaultStatus, vaultUnlock } from '#lib/api/index.js';
	import Logo from '#lib/components/Logo.svelte';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText } from '#lib/session.svelte.js';

	let password = $state('');
	let error = $state('');
	let busy = $state(false);
	let waitUntil = $state(0);
	let now = $state(Date.now());
	let timer: ReturnType<typeof setInterval> | undefined;

	let waitSeconds = $derived(Math.max(0, Math.ceil((waitUntil - now) / 1000)));

	function startWait(ms: number | undefined) {
		if (!ms) return;
		waitUntil = Date.now() + ms;
		now = Date.now();
		clearInterval(timer);
		timer = setInterval(() => {
			now = Date.now();
			if (now >= waitUntil) clearInterval(timer);
		}, 250);
	}

	onMount(async () => {
		const status = await vaultStatus().catch(() => null);
		if (status?.state === 'unlocked') return goto(resolve('/timeline'), { replace: true });
		if (status?.state === 'no_vault' || status?.state === 'creating') {
			return goto(resolve('/setup'), { replace: true });
		}
		startWait(status?.retry_after_ms);
	});

	onDestroy(() => clearInterval(timer));

	async function unlock(event: SubmitEvent) {
		event.preventDefault();
		if (busy || waitSeconds > 0 || password.length === 0) return;
		busy = true;
		error = '';
		try {
			await vaultUnlock(password);
			password = '';
			await goto(resolve('/timeline'), { replace: true });
		} catch (e) {
			error = errorText(e);
			if (isAppError(e)) startWait(e.retry_after_ms);
		} finally {
			busy = false;
		}
	}
</script>

<main class="auth">
	<div class="brand">
		<Logo />
		<h1>{t('lock.title')}</h1>
		<p class="muted">{t('lock.subtitle')}</p>
	</div>
	<form class="stack" onsubmit={unlock}>
		<PasswordField
			id="password"
			label={t('common.password')}
			bind:value={password}
			autocomplete="current-password"
			autofocus
			describedby="lock-status"
		/>
		<div id="lock-status" aria-live="polite" class="stack status">
			{#if error}<p class="notice">{error}</p>{/if}
			{#if waitSeconds > 0}<p class="muted small">
					{t('lock.wait', { seconds: waitSeconds })}
				</p>{/if}
		</div>
		<button type="submit" disabled={busy || waitSeconds > 0 || password.length === 0}>
			{busy ? t('common.working') : t('lock.unlock')}
		</button>
	</form>
	<a class="button link forgot" href={resolve('/recover')}>{t('lock.forgot')}</a>
	<!-- P-3/P-4: crisis numbers and exercises without unlocking. -->
	<a class="button secondary help" href={resolve('/help')}><LifeBuoy size={18} />{t('help.open')}</a
	>
</main>

<style>
	.status:empty {
		display: none;
	}
	.status p {
		margin: 0;
	}
	.forgot {
		align-self: center;
	}
	.help {
		margin-top: 1.5rem;
	}
</style>

<script lang="ts">
	// Settings → Sync (D-22): the folder Syncthing carries between the user's
	// devices, the devices seen in it, and joining a journal kept there by
	// another device. Mossbook itself never connects to the network.
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import { onMount } from 'svelte';

	import {
		syncChooseFolder,
		syncDisconnect,
		syncJoin,
		syncNow,
		syncStatus,
		type SyncStatus
	} from '#lib/api/index.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	import ConfirmAction from './ConfirmAction.svelte';
	import PasswordField from './PasswordField.svelte';

	let status = $state<SyncStatus | null>(null);
	let message = $state('');
	let busy = $state(false);
	let joinPassword = $state('');

	let format = $derived(
		new Intl.DateTimeFormat(locale(), { dateStyle: 'medium', timeStyle: 'short' })
	);
	const when = (at: string) => format.format(new Date(at));

	onMount(() => {
		syncStatus()
			.then((s) => (status = s))
			.catch(() => {});
	});

	async function run(action: () => Promise<SyncStatus | null | void>, done = '') {
		busy = true;
		message = '';
		try {
			const result = await action();
			if (result) status = result;
			if (result !== null) message = done;
		} catch (e) {
			if (!(await handleLocked(e))) message = errorText(e);
		} finally {
			busy = false;
		}
	}

	async function join() {
		await run(async () => {
			const result = await syncJoin(joinPassword);
			joinPassword = '';
			return result;
		}, t('sync.joined'));
	}
</script>

<section class="card stack" aria-labelledby="sync">
	<h2 id="sync">{t('sync.title')}</h2>
	<p class="muted">{t('sync.intro')}</p>

	{#if status && !status.location}
		<p class="muted small">{t('sync.howTo')}</p>
		<button type="button" class="secondary" disabled={busy} onclick={() => run(syncChooseFolder)}>
			<FolderOpen size={18} />{t('sync.choose')}
		</button>
	{:else if status}
		<p class="small path">{t('sync.folder', { folder: status.location ?? '' })}</p>
		<ul class="devices">
			{#each status.devices as device, i (i)}
				<li>
					<strong>
						{device.this_device
							? t('sync.thisDevice')
							: device.other_journal
								? t('sync.otherJournalDevice')
								: t('sync.otherDevice')}
					</strong>
					<span class="muted small">{t('sync.changedAt', { time: when(device.changed_at) })}</span>
				</li>
			{/each}
		</ul>
		<p class="muted small">
			{status.last_sync ? t('sync.lastSync', { time: when(status.last_sync) }) : t('sync.never')}
		</p>
		{#if status.problem}
			<p class="notice small" role="alert">{t(`sync.problem_${status.problem}`)}</p>
		{/if}

		{#if status.other_journal}
			<div class="join stack" role="group" aria-labelledby="sync-join">
				<h3 id="sync-join">{t('sync.joinTitle')}</h3>
				<p>{t('sync.joinIntro')}</p>
				<p class="notice">{t('sync.joinWarning')}</p>
				<PasswordField
					id="sync-join-password"
					label={t('sync.joinPassword')}
					bind:value={joinPassword}
					autocomplete="current-password"
				/>
				{#if joinPassword.length > 0}
					<ConfirmAction
						label={t('sync.join')}
						question={t('sync.joinQuestion')}
						onconfirm={join}
					/>
				{/if}
			</div>
		{/if}

		<button
			type="button"
			class="secondary"
			disabled={busy}
			onclick={() => run(syncNow, t('sync.done'))}
		>
			<RefreshCw size={18} />{t('sync.now')}
		</button>
		<p class="muted small">{t('sync.hint')}</p>
		<ConfirmAction
			label={t('sync.off')}
			question={t('sync.offQuestion')}
			onconfirm={() =>
				run(async () => {
					await syncDisconnect();
					status = await syncStatus();
				}, t('sync.offDone'))}
		/>
	{/if}
	{#if message}<p class="muted" aria-live="polite">{message}</p>{/if}
</section>

<style>
	.path {
		overflow-wrap: anywhere;
	}
	.devices {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 0.4rem;
	}
	.devices li {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: baseline;
	}
	.join {
		border-left: 3px solid var(--accent);
		padding-left: 0.75rem;
	}
	h3 {
		margin: 0;
		font-size: 1rem;
	}
</style>

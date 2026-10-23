<script lang="ts">
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import { onMount } from 'svelte';

	import {
		backupExport,
		backupInfo,
		backupReveal,
		exportReadable,
		settingsGet,
		settingsSave
	} from '#lib/api/index.js';
	import type { BackupInfo, ImportOutcome, Settings } from '#lib/api/index.js';
	import BackupImport from '#lib/components/BackupImport.svelte';
	import ConfirmAction from '#lib/components/ConfirmAction.svelte';
	import SyncSettings from '#lib/components/SyncSettings.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { createMessages } from '#lib/messages.svelte.js';

	const { messages, attempt } = createMessages();
	let settings = $state<Settings | null>(null);
	let backups = $state<BackupInfo | null>(null);

	function formatTime(timestamp: string) {
		return new Intl.DateTimeFormat(locale(), { dateStyle: 'long', timeStyle: 'short' }).format(
			new Date(timestamp)
		);
	}

	async function refreshBackups() {
		backups = await backupInfo();
	}

	onMount(() => {
		void attempt('settings', async () => {
			settings = await settingsGet();
			await refreshBackups();
		});
	});

	function saveAutoBackup(value: boolean) {
		if (!settings) return;
		const next = { ...$state.snapshot(settings), auto_backup: value } as Settings;
		void attempt('auto', async () => {
			settings = await settingsSave(next);
			await refreshBackups();
		});
	}

	function exportBackup() {
		return attempt('export', async () => {
			if (!(await backupExport())) return '';
			await refreshBackups();
			return t('backup.exported');
		});
	}

	function onImported(outcome: ImportOutcome) {
		const r = outcome.report;
		messages.import =
			t('backup.report', { added: r.added, updated: r.updated, unchanged: r.unchanged }) +
			(r.skipped > 0 ? ` ${t('backup.skipped', { skipped: r.skipped })}` : '');
	}
</script>

<UnlockedShell title={t('settings.data')} back="/settings">
	<div class="stack">
		{#if messages.settings}<p class="notice" aria-live="polite">{messages.settings}</p>{/if}
		<section class="card stack" aria-labelledby="backup">
			<h2 id="backup">{t('backup.title')}</h2>
			<p class="muted">{t('backup.intro')}</p>
			<button type="button" onclick={exportBackup}>{t('backup.export')}</button>
			{#if backups}
				<p class="muted small" data-testid="last-export">
					{backups.last_export
						? t('backup.lastExport', { date: formatTime(backups.last_export) })
						: t('backup.lastExportNever')}
				</p>
			{/if}
			{#if messages.export}<p class="muted" aria-live="polite">{messages.export}</p>{/if}
			<BackupImport unlocked={true} ondone={onImported} />
			{#if messages.import}<p class="notice" aria-live="polite">{messages.import}</p>{/if}

			<h3>{t('backup.autoTitle')}</h3>
			{#if settings}
				<label class="toggle">
					<input
						type="checkbox"
						checked={settings.auto_backup}
						onchange={(e) => saveAutoBackup(e.currentTarget.checked)}
					/>
					{t('backup.autoToggle')}
				</label>
			{/if}
			{#if backups?.auto_enabled}
				<p class="muted" data-testid="auto-latest">
					{backups.auto_latest
						? t('backup.autoLatest', { date: formatTime(backups.auto_latest) })
						: t('backup.autoNone')}
				</p>
				{#if backups.auto_failed}<p class="notice">{t('backup.autoFailed')}</p>{/if}
				{#if backups.auto_latest && backups.can_reveal}
					<button
						type="button"
						class="secondary"
						onclick={() => attempt('auto', async () => void (await backupReveal()))}
					>
						<FolderOpen size={18} />{t('backup.autoShow')}
					</button>
				{/if}
			{/if}
			<p class="muted small">{t('backup.autoHint')}</p>
			{#if messages.auto}<p class="notice" aria-live="polite">{messages.auto}</p>{/if}
		</section>

		<SyncSettings />

		<section class="card stack" aria-labelledby="export-readable">
			<h2 id="export-readable">{t('exportReadable.title')}</h2>
			<p class="muted">{t('exportReadable.intro')}</p>
			<p class="notice">{t('exportReadable.warning')}</p>
			<ConfirmAction
				label={t('exportReadable.submit')}
				question={t('exportReadable.question')}
				onconfirm={() =>
					attempt('json', async () => ((await exportReadable()) ? t('exportReadable.saved') : ''))}
			/>
			{#if messages.json}<p class="muted" aria-live="polite">{messages.json}</p>{/if}
		</section>
	</div>
</UnlockedShell>

<style>
	h2 {
		margin: 0;
		font-size: 1.15rem;
	}
	h3 {
		margin: 0.75rem 0 0;
		font-size: 1rem;
	}
	.toggle {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		min-height: var(--tap);
		font-weight: 500;
	}
	.toggle input {
		width: 1.3rem;
		min-height: 1.3rem;
		flex: none;
	}
</style>

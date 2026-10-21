<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import FileText from '@lucide/svelte/icons/file-text';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import Tag from '@lucide/svelte/icons/tag';
	import { onMount } from 'svelte';

	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		appInfo,
		backupExport,
		backupInfo,
		backupReveal,
		exportJson,
		isAppError,
		openReleases,
		settingsGet,
		settingsSave,
		vaultChangePassword,
		vaultDestroy,
		vaultVerifyRecovery
	} from '#lib/api/index.js';
	import type { AppInfo, AutoLock, BackupInfo, ImportOutcome, Settings } from '#lib/api/index.js';
	import BackupImport from '#lib/components/BackupImport.svelte';
	import ConfirmAction from '#lib/components/ConfirmAction.svelte';
	import FingerprintUnlock from '#lib/components/FingerprintUnlock.svelte';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import StrengthMeter from '#lib/components/StrengthMeter.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { detectLocale, locale, setLocale, t } from '#lib/i18n/index.svelte.js';
	import { MIN_PASSWORD_CHARS } from '#lib/password.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	const AUTOLOCK: AutoLock[] = ['immediately', 'after1_min', 'after5_min', 'after15_min'];
	/** Each language in its own name, so it can be found whatever is shown now. */
	const LANGUAGES: Array<{ value: Settings['locale']; name: string | null }> = [
		{ value: null, name: null },
		{ value: 'pl', name: 'Polski' },
		{ value: 'en', name: 'English' },
		{ value: 'uk', name: 'Українська' }
	];

	let settings = $state<Settings | null>(null);
	let info = $state<AppInfo | null>(null);
	let backups = $state<BackupInfo | null>(null);
	let recoveryPhrase = $state('');
	/** Offline nudge: no network, just the age of this build (D-15). */
	const OLD_AFTER_DAYS = 90;
	let isOld = $derived(!!info && Date.now() / 1000 - info.built_at > OLD_AFTER_DAYS * 86_400);
	/** One message line per section. */
	let messages = $state<Record<string, string>>({});

	let oldPassword = $state('');
	let newPassword = $state('');
	let repeat = $state('');
	let destroyPassword = $state('');

	let passwordReady = $derived(
		oldPassword.length > 0 &&
			[...newPassword].length >= MIN_PASSWORD_CHARS &&
			newPassword === repeat
	);

	async function attempt(section: string, action: () => Promise<string | void>) {
		messages[section] = '';
		try {
			messages[section] = (await action()) ?? '';
		} catch (e) {
			if (!(await handleLocked(e))) messages[section] = errorText(e);
		}
	}

	function formatTime(timestamp: string) {
		return new Intl.DateTimeFormat(locale(), { dateStyle: 'long', timeStyle: 'short' }).format(
			new Date(timestamp)
		);
	}

	async function refreshBackups() {
		backups = await backupInfo();
	}

	onMount(() => {
		appInfo()
			.then((i) => (info = i))
			.catch(() => {});
		return void attempt('settings', async () => {
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

	function onImported(outcome: ImportOutcome) {
		const r = outcome.report;
		messages.import =
			t('backup.report', { added: r.added, updated: r.updated, unchanged: r.unchanged }) +
			(r.skipped > 0 ? ` ${t('backup.skipped', { skipped: r.skipped })}` : '');
	}

	function changePassword(e: SubmitEvent) {
		e.preventDefault();
		if (!passwordReady) return;
		void attempt('password', async () => {
			await vaultChangePassword(oldPassword, newPassword);
			oldPassword = newPassword = repeat = '';
			await refreshBackups();
			const auto = settings?.auto_backup ? ` ${t('password.autoReplaced')}` : '';
			return `${t('password.changed')} ${t('password.oldBackups')}${auto}`;
		});
	}

	function destroy() {
		return attempt('destroy', async () => {
			await vaultDestroy(destroyPassword);
			destroyPassword = '';
			await goto(resolve('/setup'), { replace: true });
		});
	}
</script>

<UnlockedShell title={t('settings.title')}>
	<div class="stack">
		<a class="card link-row" href={resolve('/report')}>
			<FileText size={20} />
			<span>{t('report.open')}</span>
			<ChevronRight size={20} />
		</a>

		<a class="card link-row" href={resolve('/tags')}>
			<Tag size={20} />
			<span>{t('settings.tags')}</span>
			<ChevronRight size={20} />
		</a>

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

		<section class="card stack" aria-labelledby="export-json">
			<h2 id="export-json">{t('exportJson.title')}</h2>
			<p class="muted">{t('exportJson.intro')}</p>
			<p class="notice">{t('exportJson.warning')}</p>
			<ConfirmAction
				label={t('exportJson.submit')}
				question={t('exportJson.question')}
				onconfirm={() =>
					attempt('json', async () => ((await exportJson()) ? t('exportJson.saved') : ''))}
			/>
			{#if messages.json}<p class="muted" aria-live="polite">{messages.json}</p>{/if}
		</section>

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
		</section>

		<section class="card stack" aria-labelledby="destroy">
			<h2 id="destroy">{t('destroy.title')}</h2>
			<p class="muted">{t('destroy.intro')}</p>
			<PasswordField
				id="destroy-password"
				label={t('destroy.confirmLabel')}
				bind:value={destroyPassword}
				autocomplete="current-password"
			/>
			{#if destroyPassword}
				<ConfirmAction
					label={t('destroy.submit')}
					question={t('destroy.question')}
					onconfirm={destroy}
				/>
			{/if}
			{#if messages.destroy}<p class="notice" aria-live="polite">{messages.destroy}</p>{/if}
		</section>
	</div>
</UnlockedShell>

<style>
	.link-row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-height: 56px;
		color: inherit;
		font-weight: 600;
		text-decoration: none;
	}
	.link-row span {
		flex: 1;
	}
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

<script lang="ts">
	// Pick a .mossbook file, open it with its password or recovery key, and
	// restore it (no vault yet) or import it into the open journal (X-2).
	import { backupImport, backupPick } from '#lib/api/index.js';
	import type { BackupSecret, BackupSummary, ImportMode, ImportOutcome } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	import PasswordField from './PasswordField.svelte';

	interface Props {
		/** Importing into an unlocked journal (shows merge/replace). */
		unlocked: boolean;
		ondone: (outcome: ImportOutcome) => void;
	}

	let { unlocked, ondone }: Props = $props();

	let summary = $state<BackupSummary | null>(null);
	let kind = $state<BackupSecret>('password');
	let secret = $state('');
	let mode = $state<ImportMode>('merge');
	let error = $state('');
	let busy = $state(false);

	async function pick() {
		error = '';
		try {
			const picked = await backupPick();
			if (picked) {
				summary = picked;
				secret = '';
			}
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (!summary || !secret.trim() || busy) return;
		busy = true;
		error = '';
		try {
			const outcome = await backupImport(secret, kind, mode);
			secret = '';
			summary = null;
			ondone(outcome);
		} catch (err) {
			if (!(await handleLocked(err))) error = errorText(err);
		} finally {
			busy = false;
		}
	}
</script>

<div class="stack">
	<button type="button" class="secondary" onclick={pick}>
		{summary ? t('backup.choose') : unlocked ? t('backup.import') : t('backup.choose')}
	</button>

	{#if summary}
		<form class="stack" onsubmit={submit}>
			<p class="muted">{t('backup.picked', { records: summary.records })}</p>
			{#if summary.same_vault}<p class="muted">{t('backup.sameVault')}</p>{/if}

			<fieldset class="choices">
				<legend>{t('backup.secretKind')}</legend>
				<label
					><input type="radio" bind:group={kind} value="password" />
					{t('backup.usePassword')}</label
				>
				<label
					><input type="radio" bind:group={kind} value="recovery_phrase" />
					{t('backup.useRecovery')}</label
				>
			</fieldset>

			{#if kind === 'password'}
				<PasswordField
					id="backup-secret"
					label={t('backup.password')}
					bind:value={secret}
					autocomplete="current-password"
				/>
			{:else}
				<div>
					<label for="backup-phrase">{t('backup.phrase')}</label>
					<textarea
						id="backup-phrase"
						bind:value={secret}
						autocomplete="off"
						autocapitalize="off"
						spellcheck="false"></textarea>
				</div>
			{/if}

			{#if unlocked}
				<fieldset class="choices">
					<legend>{t('backup.mode')}</legend>
					<label><input type="radio" bind:group={mode} value="merge" /> {t('backup.merge')}</label>
					<label
						><input type="radio" bind:group={mode} value="replace" /> {t('backup.replace')}</label
					>
				</fieldset>
				{#if mode === 'replace'}<p class="notice">{t('backup.replaceWarning')}</p>{/if}
			{/if}

			<button type="submit" disabled={!secret.trim() || busy}>
				{busy ? t('common.working') : t('backup.submit')}
			</button>
		</form>
	{/if}

	{#if error}<p class="notice" aria-live="polite">{error}</p>{/if}
</div>

<style>
	.choices {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		border: 0;
		margin: 0;
		padding: 0;
	}
	.choices legend {
		font-weight: 600;
		margin-bottom: 0.25rem;
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

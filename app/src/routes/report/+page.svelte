<script lang="ts">
	// Report for a therapist (X-3): choose, preview, then save a PDF, optionally
	// encrypted with a password (AES-256).
	import FileDown from '@lucide/svelte/icons/file-down';
	import { onMount } from 'svelte';

	import { entriesList, reportPdf, tagsList } from '#lib/api/index.js';
	import type { Entry, EntryKindTag, Tag } from '#lib/api/index.js';
	import { addDays, localToday } from '#lib/chart.js';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import ReportPreview from '#lib/components/ReportPreview.svelte';
	import StrengthMeter from '#lib/components/StrengthMeter.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { buildReport } from '#lib/report.js';
	import { copyWithClear, errorText, handleLocked } from '#lib/session.svelte.js';
	import { endOfDay, startOfDay } from '#lib/time.js';

	const KINDS: EntryKindTag[] = ['mood', 'fear', 'event', 'sleep', 'nightmare', 'note'];
	/** Mirrors MIN_REPORT_PASSWORD_CHARS in the core. */
	const MIN_PASSWORD = 12;

	let from = $state(addDays(localToday(), -30));
	let to = $state(localToday());
	let kinds = $state<EntryKindTag[]>([...KINDS]);
	let tagFilter = $state<string[]>([]);
	let tags = $state<Tag[]>([]);
	let entries = $state<Entry[] | null>(null);
	let message = $state('');
	let busy = $state(false);
	// The text of voice notes (E-19) goes in only when the user chooses it.
	let transcripts = $state(false);

	// X-3: optional AES-256 encryption of the PDF.
	let protect = $state(false);
	let password = $state('');
	let repeat = $state('');
	let copied = $state(false);
	let copyError = $state('');

	let tagMap = $derived(new Map(tags.map((tag) => [tag.id, tag])));
	let anyTranscript = $derived(entries?.some((e) => e.voice?.transcript?.trim()) ?? false);
	let longEnough = $derived([...password].length >= MIN_PASSWORD);
	let mismatch = $derived(protect && repeat.length > 0 && password !== repeat);
	let passwordReady = $derived(!protect || (longEnough && password === repeat));
	let doc = $derived(
		entries && entries.length > 0
			? buildReport(entries, {
					from,
					to,
					tags: tagMap,
					t,
					locale: locale(),
					encrypted: protect,
					transcripts: transcripts && anyTranscript
				})
			: null
	);

	const toggle = <T,>(list: T[], item: T) =>
		list.includes(item) ? list.filter((i) => i !== item) : [...list, item];

	onMount(async () => {
		tags = await tagsList().catch(() => []);
	});

	$effect(() => {
		const filter = {
			kinds: kinds.length > 0 ? [...kinds] : KINDS,
			tags: [...tagFilter],
			from: from ? startOfDay(from) : undefined,
			to: to ? endOfDay(to) : undefined
		};
		const timer = setTimeout(async () => {
			try {
				entries = kinds.length > 0 ? await entriesList(filter) : [];
			} catch (e) {
				if (!(await handleLocked(e))) message = errorText(e);
			}
		}, 150);
		return () => clearTimeout(timer);
	});

	/** A 20-character password without visually ambiguous characters (~116 bits). */
	function generatePassword() {
		const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789';
		const bytes = new Uint32Array(20);
		crypto.getRandomValues(bytes);
		password = Array.from(bytes, (n) => alphabet[n % alphabet.length]).join('');
		repeat = password;
		copied = false;
		copyError = '';
	}

	async function copyPassword() {
		copied = false;
		copyError = '';
		try {
			await copyWithClear(password);
			copied = true;
			setTimeout(() => (copied = false), 4000);
		} catch {
			copyError = t('report.copyFailed');
		}
	}

	async function save() {
		if (!doc || busy || !passwordReady) return;
		busy = true;
		message = '';
		try {
			if (await reportPdf($state.snapshot(doc), protect ? password : undefined))
				message = t(protect ? 'report.savedEncrypted' : 'report.saved');
		} catch (e) {
			if (!(await handleLocked(e))) message = errorText(e);
		} finally {
			busy = false;
		}
	}
</script>

<UnlockedShell title={t('report.title')} back="/settings">
	<div class="stack">
		<p class="muted">{t('report.intro')}</p>

		<div class="card stack">
			<div class="dates">
				<div>
					<label for="from">{t('filter.from')}</label>
					<input id="from" type="date" bind:value={from} max={to} />
				</div>
				<div>
					<label for="to">{t('filter.to')}</label>
					<input id="to" type="date" bind:value={to} min={from} />
				</div>
			</div>
			<div>
				<p class="eyebrow">{t('report.kinds')}</p>
				<div class="chips" role="group" aria-label={t('report.kinds')}>
					{#each KINDS as kind (kind)}
						<button
							type="button"
							class="chip"
							aria-pressed={kinds.includes(kind)}
							onclick={() => (kinds = toggle(kinds, kind))}>{t(`kind.${kind}`)}</button
						>
					{/each}
				</div>
			</div>
			{#if tags.length > 0}
				<div>
					<p class="eyebrow">{t('report.tags')}</p>
					<div class="chips" role="group" aria-label={t('report.tags')}>
						{#each tags as tag (tag.id)}
							<button
								type="button"
								class="chip"
								aria-pressed={tagFilter.includes(tag.id)}
								onclick={() => (tagFilter = toggle(tagFilter, tag.id))}
							>
								<span class="dot" style:background={tag.color}></span>{tag.name}
							</button>
						{/each}
					</div>
				</div>
			{/if}
			{#if anyTranscript}
				<div>
					<label class="check">
						<input type="checkbox" bind:checked={transcripts} />
						<span>{t('report.transcripts')}</span>
					</label>
					<p class="muted small">{t('report.transcriptsHint')}</p>
				</div>
			{/if}
		</div>

		{#if doc}
			<section class="card stack" aria-labelledby="protection">
				<h2 id="protection">{t('report.protection')}</h2>
				<label class="check">
					<input type="checkbox" bind:checked={protect} />
					<span>{t('report.protect')}</span>
				</label>
				{#if protect}
					<p class="muted small">{t('report.protectHint')}</p>
					<div>
						<PasswordField
							id="report-password"
							label={t('report.password')}
							bind:value={password}
							autocomplete="new-password"
							describedby="report-password-strength"
						/>
						<StrengthMeter id="report-password-strength" {password} />
					</div>
					<PasswordField
						id="report-password-repeat"
						label={t('report.passwordRepeat')}
						bind:value={repeat}
						autocomplete="new-password"
					/>
					{#if mismatch}<p class="notice">{t('report.passwordMismatch')}</p>{/if}
					{#if password.length > 0 && !longEnough}
						<p class="notice">{t('report.passwordShort')}</p>
					{/if}
					<div class="password-actions">
						<button type="button" class="secondary" onclick={generatePassword}>
							{t('report.generate')}
						</button>
						<button
							type="button"
							class="secondary"
							onclick={copyPassword}
							disabled={password.length === 0}
						>
							{t('report.copy')}
						</button>
					</div>
					{#if copied}<p class="muted small" aria-live="polite">{t('report.copied')}</p>{/if}
					{#if copyError}<p class="notice" aria-live="polite">{copyError}</p>{/if}
					<p class="muted small">{t('report.passwordHelp')}</p>
					<p class="notice">{t('report.passwordWarning')}</p>
				{/if}
			</section>
		{/if}

		<h2>{t('report.preview')}</h2>
		{#if entries && !doc}
			<p class="muted">{t('report.empty')}</p>
		{:else if doc}
			<ReportPreview {doc} />
			{#if !protect}<p class="notice">{t('report.warning')}</p>{/if}
			<button type="button" onclick={save} disabled={busy || !passwordReady}>
				<FileDown size={20} />{busy
					? t('common.working')
					: protect
						? t('report.saveEncrypted')
						: t('report.save')}
			</button>
		{/if}
		{#if message}<p class="notice" aria-live="polite">{message}</p>{/if}
	</div>
</UnlockedShell>

<style>
	.dates {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}
	.eyebrow {
		margin-bottom: 0.5rem;
	}
	.dot {
		width: 0.6rem;
		height: 0.6rem;
		border-radius: 50%;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		min-height: var(--tap);
		font-weight: 500;
	}
	.check input {
		width: 1.3rem;
		min-height: 1.3rem;
		flex: none;
	}
	.password-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
</style>

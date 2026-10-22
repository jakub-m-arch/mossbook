<script lang="ts">
	// Settings → Turning recordings into text (E-19, D-23). Off until the user
	// downloads the model in the browser and picks the file; Mossbook itself
	// never connects to the network. The model belongs to this device only.
	import Download from '@lucide/svelte/icons/download';
	import FileUp from '@lucide/svelte/icons/file-up';
	import { onDestroy, onMount } from 'svelte';

	import {
		isAppError,
		onTranscriptionProgress,
		transcriptionCancel,
		transcriptionImport,
		transcriptionOpenDownload,
		transcriptionRemove,
		transcriptionStatus,
		type TranscriptionStatus
	} from '#lib/api/index.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { errorText } from '#lib/session.svelte.js';
	import { formatBytes } from '#lib/voice/format.js';

	import ConfirmAction from './ConfirmAction.svelte';

	/** The name of the file the browser downloads. */
	const FILE_NAME = 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2';

	let status = $state<TranscriptionStatus | null>(null);
	let importing = $state(false);
	let percent = $state(0);
	let message = $state('');
	let stopListening: (() => void) | undefined;

	const size = (bytes: number) => formatBytes(bytes, locale());

	onMount(() => {
		void refresh();
	});

	onDestroy(() => stopListening?.());

	async function refresh() {
		try {
			status = await transcriptionStatus();
		} catch {
			status = null;
		}
	}

	async function openDownload() {
		message = '';
		try {
			await transcriptionOpenDownload();
		} catch (e) {
			message = errorText(e);
		}
	}

	async function importModel() {
		message = '';
		percent = 0;
		importing = true;
		stopListening = await onTranscriptionProgress(({ done, total }) => {
			percent = total > 0 ? Math.floor((done * 100) / total) : 0;
		});
		try {
			if (await transcriptionImport()) message = t('transcription.imported');
		} catch (e) {
			if (!(isAppError(e) && e.code === 'cancelled')) message = errorText(e);
		} finally {
			stopListening?.();
			stopListening = undefined;
			importing = false;
			await refresh();
		}
	}

	async function remove() {
		message = '';
		try {
			await transcriptionRemove();
			message = t('transcription.removed');
		} catch (e) {
			message = errorText(e);
		}
		await refresh();
	}
</script>

<section class="card stack" aria-labelledby="transcription">
	<h2 id="transcription">{t('transcription.title')}</h2>
	<p class="muted">{t('transcription.intro')}</p>

	{#if status?.installed && !importing}
		<p>{t('transcription.on', { disk: size(status.disk_bytes) })}</p>
		<p class="muted small">{t('transcription.howTo')}</p>
		<ConfirmAction
			label={t('transcription.remove')}
			question={t('transcription.removeQuestion')}
			onconfirm={remove}
		/>
	{:else if status}
		<p class="muted small">
			{t('transcription.needs', {
				download: size(status.download_bytes),
				disk: size(status.disk_bytes)
			})}
		</p>
		{#if status.low_memory && status.total_memory}
			<p class="notice small" role="note">
				{t('transcription.lowMemory', { memory: size(status.total_memory) })}
			</p>
		{/if}
		{#if importing}
			<div class="stack" role="status">
				<span>{t('transcription.importing', { percent })}</span>
				<progress max="100" value={percent}></progress>
			</div>
			<button type="button" class="secondary" onclick={() => transcriptionCancel()}>
				{t('transcription.cancel')}
			</button>
		{:else}
			<p class="small">{t('transcription.step1')}</p>
			<button type="button" class="secondary" onclick={openDownload}>
				<Download size={18} />{t('transcription.download')}
			</button>
			<p class="small file">{t('transcription.step2', { file: FILE_NAME })}</p>
			<button type="button" class="secondary" disabled={status.busy} onclick={importModel}>
				<FileUp size={18} />{t('transcription.import')}
			</button>
		{/if}
	{/if}
	<p class="muted small">{t('transcription.credit')}</p>
	{#if message}<p class="muted" aria-live="polite">{message}</p>{/if}
</section>

<style>
	.file {
		overflow-wrap: anywhere;
	}
	progress {
		width: 100%;
		accent-color: var(--accent);
	}
</style>

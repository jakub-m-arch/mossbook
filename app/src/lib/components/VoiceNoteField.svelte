<script lang="ts">
	// The voice note of an entry (E-18, D-21): record up to five minutes, listen,
	// record again or remove. A finished recording is stored (encrypted) at once
	// and the form is kept, so a lock right after does not lose it (D-20).
	// Where the user imported the model, it can be turned into text on the
	// device (E-19, D-23); the text is shown for checking and saved with the entry.
	// Removing or replacing a recording with text asks whether the text moves
	// to the entry's note or goes with the recording.
	import Captions from '@lucide/svelte/icons/captions';
	import Mic from '@lucide/svelte/icons/mic';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import Square from '@lucide/svelte/icons/square';
	import Trash from '@lucide/svelte/icons/trash';
	import { onDestroy, onMount } from 'svelte';

	import {
		isAppError,
		onTranscriptionProgress,
		transcriptionCancel,
		transcriptionStatus,
		voiceSave,
		voiceTranscribe,
		type VoiceNote
	} from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';
	import { formatClock } from '#lib/voice/format.js';
	import { VoiceRecorder, canRecord, startError, type StartError } from '#lib/voice/recorder.js';

	import Sheet from './Sheet.svelte';
	import VoicePlayer from './VoicePlayer.svelte';

	interface Props {
		value: VoiceNote | null;
		/** Called after a recording was stored or removed, to keep the form now. */
		onstored?: () => void;
		/** Called with the text of a recording the user removes but whose text they keep. */
		onkeeptext?: (text: string) => void;
	}

	let { value = $bindable(), onstored, onkeeptext }: Props = $props();

	let supported = $state<boolean | null>(null);
	let recorder: VoiceRecorder | undefined;
	let recording = $state(false);
	let saving = $state(false);
	let elapsed = $state(0);
	let ticker: ReturnType<typeof setInterval> | undefined;
	let problem = $state<StartError | 'limit' | 'removed' | 'removedKept' | 'textMoved' | null>(null);

	// Asking what happens to the text of a recording that is removed or replaced.
	let textSheet: Sheet;
	let textCancel: HTMLButtonElement;
	let pending: 'remove' | 'rerecord' | null = null;
	/** Text of the recording being replaced, moved to the note once the new one is stored. */
	let carried: string | null = null;

	// Transcription (D-23): offered only where the model is imported.
	let canTranscribe = $state(false);
	let transcribing = $state<{ done: number; total: number } | null>(null);
	let transcribeMessage = $state('');
	let stopListening: (() => void) | undefined;

	// Same split as ConfirmAction: the question up to "?" is the title.
	let textParts = $derived(t('voice.textQuestion').match(/^(.*?\?)\s*(.*)$/s));
	let textTitle = $derived(textParts ? textParts[1] : t('voice.textQuestion'));
	let textDetail = $derived(textParts ? textParts[2] : '');

	onMount(() => {
		canRecord().then((ok) => (supported = ok));
		transcriptionStatus()
			.then((s) => (canTranscribe = s.installed))
			.catch(() => {});
		// Leaving the app ends the recording; what was said so far is kept if
		// the journal is still open.
		const onHidden = () => {
			if (document.visibilityState === 'hidden' && recorder) void finish();
		};
		document.addEventListener('visibilitychange', onHidden);
		return () => document.removeEventListener('visibilitychange', onHidden);
	});

	onDestroy(() => {
		clearInterval(ticker);
		recorder?.cancel();
		recorder = undefined;
		carried = null;
		stopListening?.();
		// Leaving the form stops a transcription nobody will see.
		if (transcribing) void transcriptionCancel().catch(() => {});
	});

	async function transcribe() {
		if (!value) return;
		const id = value.id;
		transcribeMessage = '';
		transcribing = { done: 0, total: 0 };
		stopListening = await onTranscriptionProgress((p) => {
			if (transcribing) transcribing = p;
		});
		try {
			const text = await voiceTranscribe(id);
			// The recording may have been replaced meanwhile.
			if (value?.id !== id) return;
			value.transcript = text;
			if (!text) transcribeMessage = t('voice.transcriptEmpty');
			onstored?.();
		} catch (e) {
			if (await handleLocked(e)) return;
			if (!(isAppError(e) && e.code === 'cancelled')) transcribeMessage = errorText(e);
		} finally {
			stopListening?.();
			stopListening = undefined;
			transcribing = null;
		}
	}

	async function start() {
		problem = null;
		try {
			recorder = await VoiceRecorder.start(() => {
				problem = 'limit';
				void finish();
			});
		} catch (e) {
			carried = null;
			problem = startError(e);
			return;
		}
		recording = true;
		elapsed = 0;
		ticker = setInterval(() => (elapsed = recorder?.elapsedMs ?? 0), 250);
	}

	async function finish() {
		const current = recorder;
		if (!current) return;
		recorder = undefined;
		clearInterval(ticker);
		recording = false;
		saving = true;
		try {
			const bytes = await current.stop();
			value = await voiceSave(bytes);
			bytes.fill(0);
			if (carried) {
				onkeeptext?.(carried);
				if (!problem) problem = 'textMoved';
			}
			onstored?.();
		} catch (e) {
			if (!(await handleLocked(e))) problem = 'failed';
		} finally {
			carried = null;
			saving = false;
		}
	}

	function cancel() {
		recorder?.cancel();
		recorder = undefined;
		carried = null;
		clearInterval(ticker);
		recording = false;
	}

	/** Remove or record again; a recording with text asks first what happens to it. */
	function ask(action: 'remove' | 'rerecord') {
		if (value?.transcript?.trim() && onkeeptext) {
			pending = action;
			textSheet.open();
			textCancel.focus();
		} else if (action === 'remove') remove(false);
		else void start();
	}

	function choose(keep: boolean) {
		const action = pending;
		pending = null;
		textSheet.close();
		if (action === 'remove') remove(keep);
		else if (action === 'rerecord') {
			// Moved only once the new recording is stored, so stopping keeps all as it was.
			carried = keep ? (value?.transcript?.trim() ?? null) : null;
			void start();
		}
	}

	function remove(keep: boolean) {
		const text = value?.transcript?.trim();
		if (keep && text) onkeeptext?.(text);
		value = null;
		problem = keep && text ? 'removedKept' : 'removed';
		onstored?.();
	}
</script>

<section class="voice stack" aria-labelledby="voice-title">
	<p class="eyebrow" id="voice-title">{t('voice.title')}</p>

	{#if recording}
		<div class="recording" role="status">
			<span class="dot" aria-hidden="true"></span>
			<span>{t('voice.recording')}</span>
			<span class="time">{t('voice.elapsed', { time: formatClock(elapsed) })}</span>
		</div>
		<div class="row">
			<button type="button" onclick={finish}><Square size={18} />{t('voice.stop')}</button>
			<button type="button" class="secondary" onclick={cancel}>{t('voice.cancel')}</button>
		</div>
	{:else if saving}
		<p class="muted">{t('voice.saving')}</p>
	{:else if value}
		{#key value.id}<VoicePlayer note={value} />{/key}
		{#if transcribing}
			<div class="stack" role="status">
				<span>
					{t('voice.transcribing', {
						done: transcribing.done,
						total: transcribing.total || '…'
					})}
				</span>
				<progress max={transcribing.total || 1} value={transcribing.done}></progress>
			</div>
			<button type="button" class="secondary" onclick={() => transcriptionCancel()}>
				{t('voice.transcribeStop')}
			</button>
		{:else if value.transcript}
			<label class="stack transcript">
				<span>{t('voice.transcriptLabel')}</span>
				<textarea rows="4" bind:value={value.transcript}></textarea>
				<span class="muted small">{t('voice.transcriptHint')}</span>
			</label>
		{:else if canTranscribe}
			<button type="button" class="secondary" onclick={transcribe}>
				<Captions size={18} />{t('voice.transcribe')}
			</button>
		{/if}
		{#if transcribeMessage}
			<p class="muted small" role="status">{transcribeMessage}</p>
		{/if}
		{#if supported}
			<div class="row">
				<button type="button" class="secondary" onclick={() => ask('rerecord')}>
					<RotateCcw size={18} />{t('voice.rerecord')}
				</button>
				<button type="button" class="secondary" onclick={() => ask('remove')}>
					<Trash size={18} />{t('voice.remove')}
				</button>
			</div>
		{:else}
			<button type="button" class="secondary" onclick={() => ask('remove')}>
				<Trash size={18} />{t('voice.remove')}
			</button>
		{/if}
	{:else if supported}
		<button type="button" class="secondary" onclick={start}>
			<Mic size={18} />{t('voice.record')}
		</button>
		<p class="muted small">{t('voice.hint')}</p>
	{:else if supported === false}
		<p class="muted small">{t('voice.unsupported')}</p>
	{/if}

	{#if problem === 'limit'}
		<p class="muted small" role="status">{t('voice.limit')}</p>
	{:else if problem === 'removed'}
		<p class="muted small" role="status">{t('voice.removed')}</p>
	{:else if problem === 'removedKept'}
		<p class="muted small" role="status">{t('voice.removedKept')}</p>
	{:else if problem === 'textMoved'}
		<p class="muted small" role="status">{t('voice.textMoved')}</p>
	{:else if problem === 'denied'}
		<p class="notice small" role="alert">{t('voice.denied')}</p>
	{:else if problem === 'no_microphone'}
		<p class="notice small" role="alert">{t('voice.noMicrophone')}</p>
	{:else if problem === 'unsupported'}
		<p class="notice small" role="alert">{t('voice.unsupported')}</p>
	{:else if problem === 'failed'}
		<p class="notice small" role="alert">{t('voice.failed')}</p>
	{/if}
</section>

<Sheet bind:this={textSheet} title={textTitle} onclose={() => (pending = null)}>
	<p class="sheet-text">{textDetail}</p>
	<div class="stack">
		<button type="button" onclick={() => choose(true)}>{t('voice.keepText')}</button>
		<button type="button" class="secondary" onclick={() => choose(false)}>
			{t('voice.dropText')}
		</button>
		<button
			type="button"
			class="secondary"
			bind:this={textCancel}
			onclick={() => textSheet.close()}
		>
			{t('common.cancel')}
		</button>
	</div>
</Sheet>

<style>
	.sheet-text {
		margin: 0;
	}
	progress {
		width: 100%;
		accent-color: var(--accent);
	}
	.transcript span:first-child {
		font-weight: 600;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
	.recording {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		font-weight: 600;
	}
	.time {
		font-variant-numeric: tabular-nums;
		color: var(--text-muted);
		font-weight: 400;
	}
	.dot {
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 50%;
		background: var(--accent);
		animation: pulse 1.2s ease-in-out infinite;
	}
	@keyframes pulse {
		50% {
			opacity: 0.3;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.dot {
			animation: none;
		}
	}
</style>

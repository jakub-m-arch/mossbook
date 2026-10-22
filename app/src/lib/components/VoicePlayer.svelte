<script lang="ts">
	// Plays a voice note (E-18). The recording is decrypted only when asked for,
	// kept in memory as an object URL and released when the player goes away,
	// e.g. when the journal locks. Nothing is written to disk.
	import Pause from '@lucide/svelte/icons/pause';
	import Play from '@lucide/svelte/icons/play';
	import { onDestroy } from 'svelte';

	import { voiceGet, type VoiceNote } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { handleLocked } from '#lib/session.svelte.js';
	import { formatClock } from '#lib/voice/format.js';

	let { note }: { note: VoiceNote } = $props();

	let audio: HTMLAudioElement | undefined = $state();
	let url: string | undefined;
	let playing = $state(false);
	let loading = $state(false);
	let position = $state(0);
	let failed = $state(false);

	async function toggle() {
		failed = false;
		if (playing) {
			audio?.pause();
			return;
		}
		if (!url) {
			loading = true;
			try {
				const bytes = await voiceGet(note.id);
				url = URL.createObjectURL(new Blob([bytes], { type: note.mime }));
			} catch (e) {
				if (!(await handleLocked(e))) failed = true;
				return;
			} finally {
				loading = false;
			}
		}
		if (!audio || !url) return;
		if (audio.src !== url) audio.src = url;
		try {
			await audio.play();
		} catch {
			failed = true;
		}
	}

	onDestroy(() => {
		audio?.pause();
		if (url) URL.revokeObjectURL(url);
		url = undefined;
	});
</script>

<div class="player">
	<button
		type="button"
		class="secondary round"
		onclick={toggle}
		disabled={loading}
		aria-label={playing ? t('voice.pause') : t('voice.play')}
	>
		{#if playing}<Pause size={20} />{:else}<Play size={20} />{/if}
	</button>
	<span class="time" aria-live="off">
		{#if playing || position > 0}{formatClock(position * 1000)} /
		{/if}{formatClock(note.duration_ms)}
	</span>
	<audio
		bind:this={audio}
		onplay={() => (playing = true)}
		onpause={() => (playing = false)}
		onended={() => {
			playing = false;
			position = 0;
		}}
		ontimeupdate={() => (position = audio?.currentTime ?? 0)}
		onerror={() => (failed = !!url)}
	></audio>
</div>
{#if failed}<p class="notice small">{t('voice.playFailed')}</p>{/if}

<style>
	.player {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}
	.round {
		width: var(--tap);
		min-width: var(--tap);
		padding: 0;
		display: grid;
		place-items: center;
		border-radius: 50%;
	}
	.time {
		font-variant-numeric: tabular-nums;
		color: var(--text-muted);
	}
</style>

<script lang="ts">
	// Paced breathing (P-4). The circle grows on the in-breath and shrinks on
	// the out-breath; with `prefers-reduced-motion` it changes size without
	// animation and the countdown carries the rhythm.
	import { onDestroy } from 'svelte';

	import type { Phase } from '#lib/exercises.js';
	import { t } from '#lib/i18n/index.svelte.js';

	let { phases, cycles = 4 }: { phases: Phase[]; cycles?: number } = $props();

	let running = $state(false);
	let finished = $state(false);
	let cycle = $state(1);
	let index = $state(0);
	let remaining = $state(0);
	let timer: ReturnType<typeof setInterval> | undefined;

	let phase = $derived(phases[index]);
	/** Circle is large after an in-breath and during the hold that follows it. */
	let expanded = $derived.by(() => {
		if (!running) return false;
		if (phase.kind === 'inhale') return true;
		if (phase.kind === 'exhale') return false;
		const previous = phases[(index - 1 + phases.length) % phases.length];
		return previous.kind === 'inhale';
	});

	function start() {
		stop();
		running = true;
		finished = false;
		cycle = 1;
		index = 0;
		remaining = phases[0].seconds;
		timer = setInterval(tick, 1000);
	}

	function tick() {
		if (remaining > 1) {
			remaining -= 1;
			return;
		}
		if (index + 1 < phases.length) {
			index += 1;
		} else if (cycle < cycles) {
			cycle += 1;
			index = 0;
		} else {
			stop();
			finished = true;
			return;
		}
		remaining = phases[index].seconds;
	}

	function stop() {
		clearInterval(timer);
		running = false;
	}

	onDestroy(stop);
</script>

<div class="stack breathing">
	<div
		class="circle"
		class:expanded
		style:--duration="{running ? phase.seconds : 0.3}s"
		aria-hidden="true"
	></div>

	<div class="status" aria-live="polite">
		{#if running}
			<p class="phase">{t(`exercise.${phase.kind}`)}</p>
			<p class="count">{t('exercise.seconds', { n: remaining })}</p>
			<p class="muted">{t('exercise.cycle', { n: cycle, total: cycles })}</p>
		{:else if finished}
			<p>{t('exercise.done')}</p>
		{/if}
	</div>

	{#if running}
		<button type="button" class="secondary" onclick={stop}>{t('exercise.stop')}</button>
	{:else}
		<button type="button" onclick={start}>
			{finished ? t('exercise.again') : t('exercise.start')}
		</button>
	{/if}
</div>

<style>
	.breathing {
		align-items: center;
		text-align: center;
	}
	.circle {
		width: 9rem;
		height: 9rem;
		margin: 2.5rem auto;
		border-radius: 50%;
		background: var(--accent-soft);
		border: 3px solid var(--accent);
		transform: scale(0.6);
		transition: transform var(--duration) ease-in-out;
	}
	.circle.expanded {
		transform: scale(1.3);
	}
	.status {
		min-height: 7rem;
	}
	.status p {
		margin: 0.2rem 0;
	}
	.phase {
		font-size: 1.5rem;
		font-weight: 600;
	}
	.count {
		font-size: 1.2rem;
	}
	button {
		width: 100%;
	}
</style>

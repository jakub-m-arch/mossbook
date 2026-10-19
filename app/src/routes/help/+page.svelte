<script lang="ts">
	// Crisis numbers and exercises (P-3, P-4). Reachable from the lock screen:
	// static content only, no user data and no unlock needed.
	import { onMount } from 'svelte';

	import { resolve } from '$app/paths';
	import { crisisLines, dial } from '#lib/api/index.js';
	import type { CrisisDirectory } from '#lib/api/index.js';
	import { detectCountry } from '#lib/country.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';

	let countries = $state<string[]>([]);
	let country = $state<string | null>(null);
	let directory = $state<CrisisDirectory | null>(null);
	let callError = $state('');

	async function load(code: string) {
		const info = await crisisLines(code).catch(() => null);
		countries = info?.countries ?? [];
		directory = info?.directory ?? null;
	}

	onMount(async () => {
		const detected = detectCountry();
		// Without any hint the app's home market is the best guess; a known
		// foreign country without data shows a notice instead of wrong numbers.
		country = detected ?? 'PL';
		await load(country);
	});

	const text = (map: Record<string, string>) => map[locale()] ?? map.en ?? Object.values(map)[0];

	async function call(number: string) {
		callError = '';
		await dial(number).catch(() => (callError = t('help.callFailed', { number })));
	}
</script>

<main class="stack">
	<button type="button" class="secondary back" onclick={() => history.back()}>
		← {t('common.back')}
	</button>
	<h1>{t('help.title')}</h1>
	<p>{t('help.intro')}</p>

	<section class="stack" aria-labelledby="phones">
		<h2 id="phones">{t('help.phones')}</h2>
		{#if directory}
			<ul class="lines">
				{#each directory.lines as line (line.number)}
					<li class="card" class:emergency={line.emergency}>
						<p class="name">{text(line.name)}</p>
						<p class="muted">{text(line.description)}</p>
						<p class="meta muted">
							{line.hours === '24/7' ? t('help.allDay') : line.hours}{line.free
								? ` · ${t('help.free')}`
								: ''}
						</p>
						<button
							type="button"
							class:secondary={!line.emergency}
							onclick={() => call(line.number)}
						>
							{t('help.call', { number: line.number })}
						</button>
					</li>
				{/each}
			</ul>
			{#if callError}<p class="notice" aria-live="polite">{callError}</p>{/if}
			<p class="muted small">{t('help.verified', { date: directory.verified_at })}</p>
		{:else if country}
			<p class="notice">{t('help.noCountry')}</p>
		{/if}
		{#if countries.length > 0}
			<div>
				<label for="country">{t('help.country')}</label>
				<select id="country" bind:value={country} onchange={() => country && load(country)}>
					{#if country && !countries.includes(country)}<option value={country}>{country}</option
						>{/if}
					{#each countries as code (code)}
						<option value={code}>{code}</option>
					{/each}
				</select>
			</div>
		{/if}
	</section>

	<section class="stack" aria-labelledby="exercises">
		<h2 id="exercises">{t('help.exercises')}</h2>
		<a class="button secondary" href={resolve('/help/breathe?pattern=478')}
			>{t('exercise.breathe478')}</a
		>
		<a class="button secondary" href={resolve('/help/breathe?pattern=box')}
			>{t('exercise.breatheBox')}</a
		>
		<a class="button secondary" href={resolve('/help/grounding')}>{t('exercise.grounding')}</a>
	</section>
</main>

<style>
	h2 {
		font-size: 1.2rem;
		margin: 0.5rem 0 0;
	}
	.back {
		align-self: flex-start;
	}
	.lines {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.lines li {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.lines p {
		margin: 0;
	}
	.lines button {
		margin-top: 0.5rem;
		font-size: 1.1rem;
	}
	.emergency {
		border-color: var(--accent);
		border-width: 2px;
	}
	.name {
		font-weight: 600;
	}
	.meta,
	.small {
		font-size: 0.85rem;
	}
	select {
		width: 100%;
		min-height: var(--tap);
		padding: 0.5rem;
		font: inherit;
		color: var(--text);
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
</style>

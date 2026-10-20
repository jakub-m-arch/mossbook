<script lang="ts">
	// A-2–A-4 as observations: counts and averages, no interpretation (SPEC §4.4).
	import type { PatternGroup, Patterns } from '#lib/api/index.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';

	import BarList, { type Bar } from './BarList.svelte';

	let { patterns }: { patterns: Patterns } = $props();

	let n = $derived(new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }));

	const groups = (list: PatternGroup[], note: 'times' | 'basis'): Bar[] =>
		list.map((g) => ({
			key: g.key,
			label: t(`insights.${g.key}`),
			value: g.average,
			display: n.format(g.average),
			note: t(`insights.${note}`, { count: g.count })
		}));

	let tags = $derived(
		patterns.top_tags.map((c) => ({
			key: c.key,
			label: c.label,
			value: c.count,
			display: String(c.count),
			note: ''
		}))
	);
	let triggers = $derived(
		patterns.top_triggers.map((c) => ({
			key: c.key,
			label: c.label,
			value: c.count,
			display: String(c.count),
			note: ''
		}))
	);
	let strategies = $derived(
		patterns.strategies.map((s) => ({
			key: s.label,
			label: s.label,
			value: Math.max(0, s.average_drop),
			display:
				s.average_drop > 0
					? t('insights.drop', { value: n.format(s.average_drop) })
					: s.average_drop < 0
						? t('insights.rise', { value: n.format(-s.average_drop) })
						: t('insights.same'),
			note: t('insights.uses', { count: s.count })
		}))
	);
	let maxCount = $derived(
		Math.max(1, ...tags.map((b) => b.value), ...triggers.map((b) => b.value))
	);
	let maxDrop = $derived(Math.max(1, ...strategies.map((b) => b.value)));
	let empty = $derived(
		tags.length +
			triggers.length +
			strategies.length +
			patterns.mood_by_sleep.length +
			patterns.mood_by_time_of_day.length ===
			0
	);
</script>

{#if !patterns.enough_data}
	<p class="muted">{t('insights.notEnough', { count: patterns.entries })}</p>
{:else if empty}
	<p class="muted">{t('insights.none')}</p>
{:else}
	<div class="stack">
		{#if patterns.mood_by_sleep.length > 0}
			<section>
				<h3>{t('insights.sleep')}</h3>
				<p class="muted small">{t('insights.sleepHint')}</p>
				<BarList
					bars={groups(patterns.mood_by_sleep, 'basis')}
					max={5}
					label={t('insights.sleep')}
				/>
			</section>
		{/if}
		{#if patterns.mood_by_time_of_day.length > 0}
			<section>
				<h3>{t('insights.timeOfDay')}</h3>
				<BarList
					bars={groups(patterns.mood_by_time_of_day, 'times')}
					max={5}
					label={t('insights.timeOfDay')}
				/>
			</section>
		{/if}
		{#if patterns.mood_by_weekday.length > 0}
			<section>
				<h3>{t('insights.weekday')}</h3>
				<BarList
					bars={groups(patterns.mood_by_weekday, 'times')}
					max={5}
					label={t('insights.weekday')}
				/>
			</section>
		{/if}
		{#if strategies.length > 0}
			<section>
				<h3>{t('insights.strategies')}</h3>
				<p class="muted small">{t('insights.strategiesHint')}</p>
				<BarList bars={strategies} max={maxDrop} label={t('insights.strategies')} />
			</section>
		{/if}
		{#if triggers.length > 0}
			<section>
				<h3>{t('insights.triggers')}</h3>
				<p class="muted small">{t('insights.triggersHint')}</p>
				<BarList bars={triggers} max={maxCount} label={t('insights.triggers')} />
			</section>
		{/if}
		{#if tags.length > 0}
			<section>
				<h3>{t('insights.tags')}</h3>
				<BarList bars={tags} max={maxCount} label={t('insights.tags')} />
			</section>
		{/if}
	</div>
{/if}

<style>
	h3 {
		margin: 0.5rem 0 0.25rem;
		font-size: 0.95rem;
	}
	section p {
		margin: 0 0 0.6rem;
	}
</style>

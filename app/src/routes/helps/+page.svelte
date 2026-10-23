<script lang="ts">
	// What has helped (D-27): the strategies from fear entries and what helped
	// in event entries, in the user's own words, with how often and when they
	// were written and, where measured, how the fear changed. Observations
	// only; each can be added to the plan for a hard moment (P-1).
	import Check from '@lucide/svelte/icons/check';
	import Plus from '@lucide/svelte/icons/plus';
	import { onMount } from 'svelte';

	import { helpsList, planGet, planSave } from '#lib/api/index.js';
	import type { Help, SafetyPlan } from '#lib/api/index.js';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	let helps = $state<Help[] | null>(null);
	let plan = $state<SafetyPlan | null>(null);
	let error = $state('');

	let dateFormat = $derived(new Intl.DateTimeFormat(locale(), { dateStyle: 'medium' }));
	let number = $derived(new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }));
	const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();
	let inPlan = $derived((label: string) => !!plan?.self_coping.some((item) => same(item, label)));

	onMount(async () => {
		try {
			[helps, plan] = await Promise.all([helpsList(), planGet()]);
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	});

	function change(help: Help): string {
		if (help.average_drop === null) return '';
		const value = number.format(Math.abs(help.average_drop));
		const key =
			help.average_drop > 0
				? 'helps.lower'
				: help.average_drop < 0
					? 'helps.higher'
					: 'helps.unchanged';
		return t(key, { value, count: help.measured });
	}

	async function addToPlan(help: Help) {
		if (!plan || inPlan(help.label)) return;
		error = '';
		try {
			const next = $state.snapshot(plan) as SafetyPlan;
			next.self_coping = [...next.self_coping.filter((item) => item.trim()), help.label];
			plan = await planSave(next);
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}
</script>

<UnlockedShell title={t('helps.title')} back="/plan">
	<div class="stack">
		<p class="muted">{t('helps.intro')}</p>
		{#if error}<p class="notice">{error}</p>{/if}

		{#if helps && helps.length === 0}
			<p class="muted">{t('helps.empty')}</p>
		{:else if helps}
			<ul class="list">
				{#each helps as help (help.label)}
					<li class="card stack help">
						<strong>{help.label}</strong>
						<span class="muted small">
							{t('helps.seen', {
								count: help.count,
								date: dateFormat.format(new Date(help.last))
							})}
						</span>
						{#if help.average_drop !== null}
							<span class="small">{change(help)}</span>
						{/if}
						{#if inPlan(help.label)}
							<span class="muted small in-plan"><Check size={16} />{t('helps.inPlan')}</span>
						{:else}
							<button type="button" class="secondary" onclick={() => addToPlan(help)}>
								<Plus size={18} />{t('helps.addToPlan')}
							</button>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</UnlockedShell>

<style>
	.list {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.help strong {
		overflow-wrap: anywhere;
	}
	.help button {
		justify-self: start;
	}
	.in-plan {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
	}
</style>

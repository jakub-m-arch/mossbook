<script lang="ts">
	// Plan for a hard moment, after the Stanley–Brown safety plan (P-1).
	import Phone from '@lucide/svelte/icons/phone';
	import { onMount } from 'svelte';

	import { resolve } from '$app/paths';
	import { dial, planGet, planSave } from '#lib/api/index.js';
	import type { Contact, SafetyPlan } from '#lib/api/index.js';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	type ListKey = 'warning_signs' | 'self_coping' | 'calming_people_places' | 'safe_environment';
	type ContactKey = 'contacts' | 'professionals';

	const SECTIONS: Array<{ key: ListKey | ContactKey; contacts: boolean }> = [
		{ key: 'warning_signs', contacts: false },
		{ key: 'self_coping', contacts: false },
		{ key: 'calming_people_places', contacts: false },
		{ key: 'contacts', contacts: true },
		{ key: 'professionals', contacts: true },
		{ key: 'safe_environment', contacts: false }
	];

	let plan = $state<SafetyPlan | null>(null);
	let error = $state('');
	let saved = $state(false);
	let busy = $state(false);

	onMount(async () => {
		try {
			plan = await planGet();
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	});

	const emptyContact = (): Contact => ({ name: '', phone: '', note: '' });

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!plan || busy) return;
		busy = true;
		error = '';
		try {
			plan = await planSave($state.snapshot(plan) as SafetyPlan);
			saved = true;
			setTimeout(() => (saved = false), 4000);
		} catch (err) {
			if (!(await handleLocked(err))) error = errorText(err);
		} finally {
			busy = false;
		}
	}

	async function call(phone: string) {
		error = '';
		await dial(phone).catch(() => (error = t('help.callFailed', { number: phone })));
	}
</script>

<UnlockedShell title={t('plan.title')}>
	<div class="stack">
		<p>{t('plan.intro')}</p>
		<a class="button secondary" href={resolve('/help')}><Phone size={18} />{t('help.open')}</a>

		{#if plan}
			<form class="stack" onsubmit={save}>
				{#each SECTIONS as section (section.key)}
					<fieldset class="card stack section">
						<legend>{t(`plan.${section.key}`)}</legend>
						{#if section.contacts}
							{@const list = plan[section.key as ContactKey]}
							{#each list as contact, i (i)}
								<div class="contact stack">
									<input
										aria-label={t('plan.name')}
										placeholder={t('plan.name')}
										bind:value={contact.name}
									/>
									<input
										aria-label={t('plan.phone')}
										placeholder={t('plan.phone')}
										type="tel"
										inputmode="tel"
										bind:value={contact.phone}
									/>
									<input
										aria-label={t('plan.note')}
										placeholder={t('plan.note')}
										bind:value={contact.note}
									/>
									<div class="row">
										{#if contact.phone.trim()}
											<button type="button" class="secondary" onclick={() => call(contact.phone)}>
												{t('help.call', { number: contact.phone })}
											</button>
										{/if}
										<button type="button" class="link" onclick={() => list.splice(i, 1)}>
											{t('plan.remove')}
										</button>
									</div>
								</div>
							{/each}
							<button type="button" class="secondary" onclick={() => list.push(emptyContact())}>
								{t('plan.add')}
							</button>
						{:else}
							{@const list = plan[section.key as ListKey]}
							{#each list.keys() as i (i)}
								<div class="row">
									<input aria-label={t('plan.item', { n: i + 1 })} bind:value={list[i]} />
									<button
										type="button"
										class="link"
										aria-label="{t('plan.remove')} {i + 1}"
										onclick={() => list.splice(i, 1)}>×</button
									>
								</div>
							{/each}
							<button type="button" class="secondary" onclick={() => list.push('')}>
								{t('plan.add')}
							</button>
						{/if}
					</fieldset>
				{/each}

				<div aria-live="polite">
					{#if saved}<p class="notice">{t('plan.saved')}</p>{/if}
					{#if error}<p class="notice">{error}</p>{/if}
				</div>
				<button type="submit" disabled={busy}>{busy ? t('common.working') : t('plan.save')}</button>
			</form>
		{:else if error}
			<p class="notice">{error}</p>
		{/if}
	</div>
</UnlockedShell>

<style>
	fieldset {
		margin: 0;
		border: 0;
	}
	legend {
		float: left;
		width: 100%;
		margin-bottom: 0.25rem;
		font-weight: 650;
	}
	.row {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		flex-wrap: wrap;
	}
	.row input {
		flex: 1;
	}
	.contact {
		padding-bottom: 0.75rem;
		border-bottom: 1px solid var(--border);
	}
</style>

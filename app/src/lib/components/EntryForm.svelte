<script lang="ts">
	import Trash from '@lucide/svelte/icons/trash';
	import { onMount, untrack } from 'svelte';

	import type {
		Emotion,
		EntryDraft,
		EntryKind,
		EntryKindTag,
		Symptom,
		Tag
	} from '#lib/api/index.js';
	import { EMOTIONS, SYMPTOMS, emptyKind, jsonCopy, kindTag } from '#lib/entries.js';
	import { locale, t } from '#lib/i18n/index.svelte.js';
	import { fromLocalInput, toLocalInput, toRfc3339 } from '#lib/time.js';
	import { setFlush } from '#lib/unsaved.js';

	import ChipSelect from './ChipSelect.svelte';
	import ConfirmAction from './ConfirmAction.svelte';
	import MoodPicker from './MoodPicker.svelte';
	import ScalePicker from './ScalePicker.svelte';
	import TagPicker from './TagPicker.svelte';
	import VoiceNoteField from './VoiceNoteField.svelte';

	interface Props {
		initial: EntryDraft;
		tags: Tag[];
		onsave: (draft: EntryDraft) => Promise<void>;
		/** Called with the form's state shortly after each change, so a lock does not lose it (D-20). */
		onchange?: (draft: EntryDraft) => void;
		/** For a saved entry: deleting it, next to saving, after a confirmation. */
		ondelete?: () => Promise<void>;
	}

	let { initial: initialProp, tags, onsave, onchange, ondelete }: Props = $props();
	let confirmDelete: ConfirmAction | undefined = $state();
	// The form is created once per entry (the page keys it by id), so it reads
	// the initial draft a single time and then owns its state.
	const initial = untrack(() => $state.snapshot(initialProp) as EntryDraft);

	// Every form can be saved with a single field filled in (SPEC §8.5).
	const tag: EntryKindTag = kindTag(initial.kind);
	const pick = <K extends EntryKindTag>(k: K) =>
		jsonCopy((tag === k ? initial.kind : emptyKind(k)) as Extract<EntryKind, Record<K, unknown>>);

	let occurredAt = $state(toLocalInput(initial.occurred_at ?? toRfc3339(new Date())));
	let mood = $state(initial.mood ?? null);
	let intensity = $state(initial.intensity ?? null);
	let note = $state(initial.note ?? '');
	let selectedTags = $state([...(initial.tags ?? [])]);

	let moodKind = $state(pick('mood').mood);
	let event = $state(pick('event').event);
	let fear = $state(pick('fear').fear);
	let sleep = $state(pick('sleep').sleep);
	let nightmare = $state(pick('nightmare').nightmare);
	let emotions = $state<Emotion[]>(moodKind.emotions);
	let symptoms = $state<Symptom[]>(fear.symptoms);
	let strategy = $state(fear.strategy ?? '');
	let bedtime = $state(sleep.bedtime ? toLocalInput(sleep.bedtime) : '');
	let wake = $state(sleep.wake ? toLocalInput(sleep.wake) : '');

	let sensitive = $state(initial.sensitive ?? false);
	let voice = $state(initial.voice ?? null);
	// An existing nightmare starts collapsed so it is not read by accident (E-5).
	let showNightmare = $state(!nightmare.content.trim());
	let dateFormat = $derived(new Intl.DateTimeFormat(locale(), { dateStyle: 'medium' }));

	let busy = $state(false);

	function rehearse() {
		nightmare.rehearsals = [...nightmare.rehearsals, toRfc3339(new Date())];
	}

	function kind(): EntryKind {
		switch (tag) {
			case 'mood':
				return { mood: { ...moodKind, emotions } };
			case 'event':
				return { event };
			case 'fear':
				return { fear: { ...fear, symptoms, strategy: strategy.trim() || null } };
			case 'sleep':
				return {
					sleep: {
						...sleep,
						bedtime: bedtime ? fromLocalInput(bedtime) : null,
						wake: wake ? fromLocalInput(wake) : null
					}
				};
			case 'nightmare':
				return { nightmare };
			case 'note':
				return 'note';
		}
	}

	function current(incomplete: boolean): EntryDraft {
		return {
			id: initial.id,
			kind: $state.snapshot(kind()) as EntryKind,
			occurred_at: occurredAt ? fromLocalInput(occurredAt) : undefined,
			mood,
			intensity: tag === 'fear' || tag === 'event' ? intensity : (initial.intensity ?? null),
			tags: $state.snapshot(selectedTags),
			note,
			incomplete,
			sensitive,
			voice: $state.snapshot(voice)
		};
	}

	/** Pause in typing after which the form is kept (D-20). */
	const KEEP_AFTER_MS = 800;
	let keepTimer: ReturnType<typeof setTimeout> | undefined;
	let opened: string | undefined;
	let changed = false;

	function keepNow() {
		clearTimeout(keepTimer);
		keepTimer = undefined;
		onchange?.(current(initial.incomplete ?? false));
	}

	// Any change after the form opened is kept once typing pauses.
	$effect(() => {
		const snapshot = JSON.stringify(current(false));
		untrack(() => {
			if (!onchange) return;
			opened ??= snapshot;
			if (!changed && snapshot === opened) return;
			changed = true;
			clearTimeout(keepTimer);
			keepTimer = setTimeout(keepNow, KEEP_AFTER_MS);
		});
	});

	onMount(() => {
		if (!onchange) return;
		const flush = () => {
			if (keepTimer !== undefined) keepNow();
		};
		// Going to the background may lock the journal right away (V-3).
		const onHidden = () => {
			if (document.visibilityState === 'hidden') flush();
		};
		setFlush(flush);
		document.addEventListener('visibilitychange', onHidden);
		return () => {
			clearTimeout(keepTimer);
			setFlush(null);
			document.removeEventListener('visibilitychange', onHidden);
		};
	});

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (busy) return;
		busy = true;
		clearTimeout(keepTimer);
		keepTimer = undefined;
		try {
			await onsave(current(false));
		} finally {
			busy = false;
		}
	}
</script>

<form class="stack" onsubmit={submit}>
	<div class="card stack">
		{#if tag === 'mood'}
			<MoodPicker bind:value={mood} />
			<ScalePicker
				label={t('scale.energy')}
				name="energy"
				min={1}
				max={5}
				bind:value={moodKind.energy}
			/>
			<ChipSelect
				label={t('emotion.label')}
				prefix="emotion"
				options={EMOTIONS}
				bind:selected={emotions}
			/>
		{:else if tag === 'fear'}
			<div>
				<label for="trigger">{t('field.trigger')}</label>
				<textarea id="trigger" bind:value={fear.trigger}></textarea>
			</div>
			<ScalePicker label={t('scale.intensity')} name="intensity" bind:value={intensity} />
			<ChipSelect
				label={t('symptom.label')}
				prefix="symptom"
				options={SYMPTOMS}
				bind:selected={symptoms}
			/>
			<fieldset class="inline">
				<legend>{t('field.avoidance')}</legend>
				{#each [true, false] as answer (answer)}
					<label class="choice">
						<input
							type="radio"
							name="avoidance"
							checked={fear.avoidance === answer}
							onclick={() => (fear.avoidance = fear.avoidance === answer ? null : answer)}
						/>
						{answer ? t('field.yes') : t('field.no')}
					</label>
				{/each}
			</fieldset>
			<div>
				<label for="strategy">{t('field.strategy')}</label>
				<input id="strategy" bind:value={strategy} />
			</div>
			<ScalePicker
				label={t('scale.after')}
				name="intensity-after"
				bind:value={fear.intensity_after}
			/>
		{:else if tag === 'event'}
			{#each [['what', true], ['place', false], ['body', true], ['thoughts', true], ['helped', true]] as const as [field, long] (field)}
				<div>
					<label for={field}>{t(`field.${field}`)}</label>
					{#if long}
						<textarea id={field} bind:value={event[field]}></textarea>
					{:else}
						<input id={field} bind:value={event[field]} />
					{/if}
				</div>
			{/each}
			<ScalePicker label={t('scale.intensity')} name="intensity" bind:value={intensity} />
		{:else if tag === 'sleep'}
			<div>
				<label for="bedtime">{t('field.bedtime')}</label>
				<input id="bedtime" type="datetime-local" bind:value={bedtime} />
			</div>
			<div>
				<label for="wake">{t('field.wake')}</label>
				<input id="wake" type="datetime-local" bind:value={wake} />
			</div>
			<ScalePicker
				label={t('scale.quality')}
				name="quality"
				min={1}
				max={5}
				bind:value={sleep.quality}
			/>
			<div>
				<label for="awakenings">{t('field.awakenings')}</label>
				<input
					id="awakenings"
					type="number"
					min="0"
					max="50"
					inputmode="numeric"
					bind:value={sleep.awakenings}
				/>
			</div>
			<label class="choice">
				<input type="checkbox" bind:checked={sleep.nightmare} />
				{t('field.nightmare')}
			</label>
		{:else if tag === 'nightmare'}
			<p class="muted small">{t('nightmare.intro')}</p>
			{#if showNightmare}
				<div>
					<label for="content">{t('field.content')}</label>
					<textarea id="content" bind:value={nightmare.content}></textarea>
				</div>
				{#if nightmare.content.trim()}
					<button type="button" class="link" onclick={() => (showNightmare = false)}>
						{t('nightmare.hideContent')}
					</button>
				{/if}
			{:else}
				<button type="button" class="secondary" onclick={() => (showNightmare = true)}>
					{t('nightmare.showContent')}
				</button>
			{/if}
			<div>
				<label for="rescripted">{t('field.rescripted')}</label>
				<textarea id="rescripted" class="tall" bind:value={nightmare.rescripted}></textarea>
			</div>
			<div>
				<p class="eyebrow">{t('field.rehearsals')}</p>
				{#if nightmare.rehearsals.length === 0}
					<p class="muted small">{t('nightmare.noRehearsals')}</p>
				{:else}
					<ul class="rehearsals">
						{#each nightmare.rehearsals as at, i (i)}
							<li>{dateFormat.format(new Date(at))}</li>
						{/each}
					</ul>
				{/if}
				<button type="button" class="secondary" onclick={rehearse}>{t('nightmare.rehearse')}</button
				>
			</div>
		{/if}

		{#if tag === 'note'}
			<div>
				<label for="note">{t('field.note')}</label>
				<textarea id="note" class="tall" bind:value={note}></textarea>
			</div>
		{/if}
	</div>

	<div class="card stack">
		{#if tag !== 'mood'}
			<MoodPicker bind:value={mood} />
		{/if}

		{#if tag !== 'note'}
			<div>
				<label for="note">{t('field.note')}</label>
				<textarea id="note" bind:value={note}></textarea>
			</div>
		{/if}

		<VoiceNoteField
			bind:value={voice}
			onstored={() => onchange && keepNow()}
			onkeeptext={(text) => (note = note.trim() ? `${note.trimEnd()}\n\n${text}` : text)}
		/>

		<TagPicker {tags} bind:selected={selectedTags} />

		<label class="choice sensitive">
			<input type="checkbox" bind:checked={sensitive} />
			<span>
				{t('field.sensitive')}
				<span class="muted small hint">{t('field.sensitiveHint')}</span>
			</span>
		</label>

		<div>
			<label for="occurred-at">{t('field.occurredAt')}</label>
			<input id="occurred-at" type="datetime-local" bind:value={occurredAt} />
		</div>
	</div>

	<div class="save-bar">
		{#if ondelete}
			<button
				type="button"
				class="secondary delete"
				disabled={busy}
				aria-label={t('entry.delete')}
				onclick={() => confirmDelete?.ask()}
			>
				<Trash size={20} />
			</button>
		{/if}
		<button type="submit" disabled={busy}>{busy ? t('common.working') : t('entry.save')}</button>
	</div>
</form>
{#if ondelete}
	<ConfirmAction
		bind:this={confirmDelete}
		button={false}
		label={t('entry.delete')}
		question={t('entry.deleteConfirm')}
		onconfirm={ondelete}
	/>
{/if}

<style>
	.rehearsals {
		margin: 0 0 0.5rem;
		padding-left: 1.2rem;
	}
	.sensitive {
		align-items: flex-start;
	}
	.sensitive .hint {
		display: block;
		font-weight: 400;
	}
	.tall {
		min-height: 12rem;
	}
	.save-bar {
		position: sticky;
		bottom: 0;
		padding: 0.75rem 0 calc(0.75rem + env(safe-area-inset-bottom));
		background: linear-gradient(to top, var(--bg) 70%, transparent);
	}
	.save-bar {
		display: flex;
		gap: 0.5rem;
	}
	.save-bar button[type='submit'] {
		flex: 1;
	}
	.save-bar .delete {
		flex: 0 0 var(--tap);
		padding: 0;
		justify-content: center;
	}
	fieldset.inline {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		border: 0;
		margin: 0;
		padding: 0;
	}
	fieldset.inline legend {
		width: 100%;
		font-size: 0.9rem;
		font-weight: 600;
		margin-bottom: 0.4rem;
	}
	.choice {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		min-height: var(--tap);
		font-weight: 500;
	}
	.choice input {
		width: 1.4rem;
		min-height: 1.4rem;
	}
</style>

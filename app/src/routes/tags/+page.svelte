<script lang="ts">
	import { onMount } from 'svelte';

	import { tagCreate, tagDelete, tagMerge, tagSave, tagsList } from '#lib/api/index.js';
	import type { Tag } from '#lib/api/index.js';
	import ConfirmAction from '#lib/components/ConfirmAction.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { errorText, handleLocked } from '#lib/session.svelte.js';

	// E-12. Calm colors that read well in light and dark mode.
	const COLORS = [
		'#4f6b47',
		'#5b7f95',
		'#8a6a2f',
		'#7a5c8e',
		'#a0675a',
		'#3f7d74',
		'#6b6f3a',
		'#7d7368'
	];

	let tags = $state<Tag[]>([]);
	let name = $state('');
	let color = $state(COLORS[0]);
	let error = $state('');
	let mergeTarget = $state<Record<string, string>>({});

	async function run(action: () => Promise<unknown>) {
		error = '';
		try {
			await action();
			tags = await tagsList();
		} catch (e) {
			if (!(await handleLocked(e))) error = errorText(e);
		}
	}

	onMount(() => run(async () => {}));

	function create(e: SubmitEvent) {
		e.preventDefault();
		if (!name.trim()) return;
		void run(async () => {
			await tagCreate(name, color);
			name = '';
		});
	}
</script>

<UnlockedShell title={t('tags.title')} back>
	<div class="stack">
		{#if error}<p class="notice" aria-live="polite">{error}</p>{/if}

		<form class="card stack" onsubmit={create}>
			<div>
				<label for="new-tag">{t('tags.name')}</label>
				<input id="new-tag" bind:value={name} maxlength="64" />
			</div>
			<fieldset class="colors">
				<legend>{t('tags.color')}</legend>
				{#each COLORS as c (c)}
					<label class="swatch" style:background={c} class:selected={color === c}>
						<input type="radio" name="new-color" value={c} bind:group={color} class="sr-only" />
						<span class="sr-only">{c}</span>
					</label>
				{/each}
			</fieldset>
			<button type="submit" disabled={!name.trim()}>{t('tags.create')}</button>
		</form>

		{#if tags.length === 0}<p class="muted">{t('tags.empty')}</p>{/if}

		<ul class="list">
			{#each tags as tag (tag.id)}
				<li class="card stack">
					<div class="row">
						<input
							aria-label={t('tags.name')}
							value={tag.name}
							maxlength="64"
							onchange={(e) => run(() => tagSave({ ...tag, name: e.currentTarget.value }))}
						/>
					</div>
					<fieldset class="colors">
						<legend class="sr-only">{t('tags.color')}</legend>
						{#each COLORS as c (c)}
							<label class="swatch" style:background={c} class:selected={tag.color === c}>
								<input
									type="radio"
									name="color-{tag.id}"
									value={c}
									checked={tag.color === c}
									onchange={() => run(() => tagSave({ ...tag, color: c }))}
									class="sr-only"
								/>
								<span class="sr-only">{c}</span>
							</label>
						{/each}
					</fieldset>
					{#if tags.length > 1}
						<div class="row">
							<select aria-label={t('tags.merge')} bind:value={mergeTarget[tag.id]}>
								<option value={undefined}>{t('tags.merge')}</option>
								{#each tags.filter((o) => o.id !== tag.id) as other (other.id)}
									<option value={other.id}>{other.name}</option>
								{/each}
							</select>
							{#if mergeTarget[tag.id]}
								<ConfirmAction
									label={t('tags.save')}
									question={t('tags.mergeConfirm')}
									onconfirm={() => run(() => tagMerge(tag.id, mergeTarget[tag.id]))}
								/>
							{/if}
						</div>
					{/if}
					<ConfirmAction
						label={t('tags.delete')}
						question={t('tags.deleteConfirm')}
						onconfirm={() => run(() => tagDelete(tag.id))}
					/>
				</li>
			{/each}
		</ul>
	</div>
</UnlockedShell>

<style>
	.list {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.row {
		display: flex;
		gap: 0.5rem;
		flex-wrap: wrap;
		align-items: center;
	}
	.row input,
	.row select {
		flex: 1;
	}
	select {
		min-height: var(--tap);
		padding: 0.5rem;
		font: inherit;
		color: var(--text);
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.colors {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		border: 0;
		margin: 0;
		padding: 0;
	}
	.colors legend {
		width: 100%;
		font-weight: 600;
		margin-bottom: 0.4rem;
	}
	.swatch {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		border: 3px solid transparent;
		cursor: pointer;
	}
	.swatch.selected {
		border-color: var(--text);
	}
	.swatch:focus-within {
		outline: 3px solid var(--focus);
		outline-offset: 2px;
	}
</style>

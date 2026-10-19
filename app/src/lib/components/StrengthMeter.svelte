<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';
	import { passwordStrength } from '#lib/password.js';

	let { password, id }: { password: string; id: string } = $props();
	let strength = $derived(passwordStrength(password));
</script>

<div {id} class="meter" aria-live="polite">
	<div class="bars" aria-hidden="true">
		{#each [1, 2, 3] as level (level)}
			<span class:on={strength >= level}></span>
		{/each}
	</div>
	<span class="muted">{t('strength.label')}: {t(`strength.${strength}`)}</span>
</div>

<style>
	.meter {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		margin-top: 0.4rem;
		font-size: 0.9rem;
	}
	.bars {
		display: flex;
		gap: 4px;
	}
	.bars span {
		width: 2rem;
		height: 6px;
		border-radius: 3px;
		background: var(--surface-2);
	}
	.bars span.on {
		background: var(--accent);
	}
</style>

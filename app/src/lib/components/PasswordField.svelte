<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';

	interface Props {
		id: string;
		label: string;
		value: string;
		autocomplete: 'current-password' | 'new-password';
		autofocus?: boolean;
		describedby?: string;
	}

	let {
		id,
		label,
		value = $bindable(),
		autocomplete,
		autofocus = false,
		describedby
	}: Props = $props();
	let visible = $state(false);
</script>

<div>
	<label for={id}>{label}</label>
	<div class="row">
		<!-- svelte-ignore a11y_autofocus -->
		<input
			{id}
			type={visible ? 'text' : 'password'}
			bind:value
			{autocomplete}
			{autofocus}
			autocapitalize="off"
			spellcheck="false"
			aria-describedby={describedby}
		/>
		<button
			type="button"
			class="secondary"
			aria-pressed={visible}
			aria-controls={id}
			onclick={() => (visible = !visible)}
		>
			{visible ? t('common.hide') : t('common.show')}
		</button>
	</div>
</div>

<style>
	.row {
		display: flex;
		gap: 0.5rem;
	}
	.row input {
		flex: 1;
	}
</style>

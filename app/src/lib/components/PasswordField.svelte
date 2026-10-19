<script lang="ts">
	import Eye from '@lucide/svelte/icons/eye';
	import EyeOff from '@lucide/svelte/icons/eye-off';

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
	<div class="field">
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
			class="icon"
			aria-pressed={visible}
			aria-controls={id}
			aria-label={visible ? t('common.hide') : t('common.show')}
			onclick={() => (visible = !visible)}
		>
			{#if visible}<EyeOff size={20} />{:else}<Eye size={20} />{/if}
		</button>
	</div>
</div>

<style>
	.field {
		position: relative;
	}
	.field input {
		padding-right: 3.25rem;
	}
	.field button {
		position: absolute;
		right: 2px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--text-muted);
	}
</style>

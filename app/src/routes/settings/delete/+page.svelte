<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { vaultDestroy } from '#lib/api/index.js';
	import ConfirmAction from '#lib/components/ConfirmAction.svelte';
	import PasswordField from '#lib/components/PasswordField.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import { createMessages } from '#lib/messages.svelte.js';

	const { messages, attempt } = createMessages();
	let destroyPassword = $state('');

	function destroy() {
		return attempt('destroy', async () => {
			await vaultDestroy(destroyPassword);
			destroyPassword = '';
			await goto(resolve('/setup'), { replace: true });
		});
	}
</script>

<UnlockedShell title={t('settings.destroy')} back="/settings">
	<section class="card stack" aria-labelledby="destroy">
		<h2 id="destroy">{t('destroy.title')}</h2>
		<p class="muted">{t('destroy.intro')}</p>
		<PasswordField
			id="destroy-password"
			label={t('destroy.confirmLabel')}
			bind:value={destroyPassword}
			autocomplete="current-password"
		/>
		{#if destroyPassword}
			<ConfirmAction
				label={t('destroy.submit')}
				question={t('destroy.question')}
				onconfirm={destroy}
			/>
		{/if}
		{#if messages.destroy}<p class="notice" aria-live="polite">{messages.destroy}</p>{/if}
	</section>
</UnlockedShell>

<style>
	h2 {
		margin: 0;
		font-size: 1.15rem;
	}
</style>

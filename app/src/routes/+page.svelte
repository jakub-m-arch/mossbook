<script lang="ts">
	import { onMount } from 'svelte';

	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { vaultStatus } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';

	// Entry point: route by vault state. A vault being created after a restart
	// starts the setup over (the phrase is never shown twice).
	onMount(async () => {
		const status = await vaultStatus().catch(() => null);
		const target =
			status?.state === 'unlocked'
				? resolve('/timeline')
				: status?.state === 'locked'
					? resolve('/lock')
					: resolve('/setup');
		await goto(target, { replaceState: true });
	});
</script>

<main>
	<p class="muted" aria-live="polite">{t('common.working')}</p>
</main>

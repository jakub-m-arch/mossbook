<script lang="ts">
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
</script>

<UnlockedShell title={t('timeline.title')}>
	<p class="muted">{t('timeline.empty')}</p>
</UnlockedShell>

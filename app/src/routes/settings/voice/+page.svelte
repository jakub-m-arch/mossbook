<script lang="ts">
	import TranscriptionSettings from '#lib/components/TranscriptionSettings.svelte';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
</script>

<UnlockedShell title={t('settings.voice')} back="/settings">
	<TranscriptionSettings />
</UnlockedShell>

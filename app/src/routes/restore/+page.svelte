<script lang="ts">
	// Restore a backup on a device without a journal (X-2).
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import BackupImport from '#lib/components/BackupImport.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
</script>

<main class="auth">
	<a class="button link back" href={resolve('/setup')}>← {t('common.back')}</a>
	<h1>{t('backup.restoreTitle')}</h1>
	<p>{t('backup.restoreIntro')}</p>
	<BackupImport unlocked={false} ondone={() => goto(resolve('/timeline'), { replace: true })} />
</main>

<style>
	.back {
		align-self: flex-start;
	}
</style>

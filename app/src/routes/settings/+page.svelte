<script lang="ts">
	// Settings: a short list of groups, each on its own screen (D-24).
	import AudioLines from '@lucide/svelte/icons/audio-lines';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import DatabaseBackup from '@lucide/svelte/icons/database-backup';
	import FileText from '@lucide/svelte/icons/file-text';
	import Info from '@lucide/svelte/icons/info';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import Languages from '@lucide/svelte/icons/languages';
	import Tag from '@lucide/svelte/icons/tag';
	import Trash from '@lucide/svelte/icons/trash';

	import { resolve } from '$app/paths';
	import UnlockedShell from '#lib/components/UnlockedShell.svelte';
	import { t } from '#lib/i18n/index.svelte.js';

	const GROUPS = [
		{
			href: '/settings/general',
			icon: Languages,
			title: 'settings.general',
			hint: 'settings.generalHint'
		},
		{
			href: '/settings/data',
			icon: DatabaseBackup,
			title: 'settings.data',
			hint: 'settings.dataHint'
		},
		{
			href: '/settings/voice',
			icon: AudioLines,
			title: 'settings.voice',
			hint: 'settings.voiceHint'
		},
		{
			href: '/settings/security',
			icon: KeyRound,
			title: 'settings.security',
			hint: 'settings.securityHint'
		},
		{ href: '/settings/about', icon: Info, title: 'settings.about', hint: 'settings.aboutHint' }
	] as const;
</script>

<UnlockedShell title={t('settings.title')}>
	<div class="stack">
		<nav class="card list" aria-label={t('settings.title')}>
			<a href={resolve('/report')}>
				<FileText size={22} />
				<span class="label"><strong>{t('report.open')}</strong></span>
				<ChevronRight size={20} />
			</a>
			<a href={resolve('/tags')}>
				<Tag size={22} />
				<span class="label"><strong>{t('settings.tags')}</strong></span>
				<ChevronRight size={20} />
			</a>
		</nav>

		<nav class="card list" aria-label={t('settings.title')}>
			{#each GROUPS as group (group.href)}
				<a href={resolve(group.href)}>
					<group.icon size={22} />
					<span class="label">
						<strong>{t(group.title)}</strong>
						<span class="muted small">{t(group.hint)}</span>
					</span>
					<ChevronRight size={20} />
				</a>
			{/each}
		</nav>

		<nav class="card list" aria-label={t('settings.destroy')}>
			<a href={resolve('/settings/delete')}>
				<Trash size={22} />
				<span class="label"><strong>{t('settings.destroy')}</strong></span>
				<ChevronRight size={20} />
			</a>
		</nav>
	</div>
</UnlockedShell>

<style>
	.list {
		display: flex;
		flex-direction: column;
		padding: 0.25rem 1rem;
	}
	.list a {
		display: flex;
		align-items: center;
		gap: 0.85rem;
		min-height: 56px;
		padding: 0.6rem 0;
		color: inherit;
		text-decoration: none;
	}
	.list a + a {
		border-top: 1px solid var(--surface-3);
	}
	.list a > :global(svg:first-child) {
		color: var(--accent);
		flex: none;
	}
	.list a > :global(svg:last-child) {
		color: var(--text-muted);
		flex: none;
	}
	.label {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
		min-width: 0;
	}
	strong {
		font-weight: 600;
	}
</style>

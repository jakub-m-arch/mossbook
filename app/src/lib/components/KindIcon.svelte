<script lang="ts">
	import Calendar from '@lucide/svelte/icons/calendar';
	import MessagesSquare from '@lucide/svelte/icons/messages-square';
	import CloudMoon from '@lucide/svelte/icons/cloud-moon';
	import CloudSun from '@lucide/svelte/icons/cloud-sun';
	import Moon from '@lucide/svelte/icons/moon';
	import PenLine from '@lucide/svelte/icons/pen-line';
	import Wind from '@lucide/svelte/icons/wind';

	import type { EntryKindTag } from '#lib/api/index.js';

	let { kind, size = 40 }: { kind: EntryKindTag; size?: number } = $props();

	const ICONS = {
		mood: CloudSun,
		event: Calendar,
		fear: Wind,
		sleep: Moon,
		note: PenLine,
		nightmare: CloudMoon,
		session: MessagesSquare
	} as const;

	let Icon = $derived(ICONS[kind]);
</script>

<span class="kind" style:--size="{size}px" style:--tint="var(--kind-{kind})" aria-hidden="true">
	<Icon size={Math.round(size * 0.5)} strokeWidth={2} />
</span>

<style>
	.kind {
		display: inline-grid;
		place-items: center;
		flex: none;
		width: var(--size);
		height: var(--size);
		border-radius: 50%;
		color: var(--tint);
		background: color-mix(in srgb, var(--tint) 14%, transparent);
	}
</style>

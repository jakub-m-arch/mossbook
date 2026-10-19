<script lang="ts">
	// Mossbook mark: a sprouting leaf on a soft moss disc.
	let { size = 56 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" class="logo">
	<circle cx="32" cy="32" r="32" class="disc" />
	<path d="M32 48V30" class="stem" />
	<path d="M32 34c0-8 6-14 15-14 0 9-6 15-15 15z" class="leaf" />
	<path d="M32 38c0-6-5-11-12-11 0 7 5 12 12 12z" class="leaf small" />
</svg>

<style>
	.disc {
		fill: var(--accent-soft);
	}
	.stem {
		stroke: var(--accent);
		stroke-width: 3;
		stroke-linecap: round;
		fill: none;
	}
	.leaf {
		fill: var(--accent);
	}
	.leaf.small {
		opacity: 0.7;
	}
</style>

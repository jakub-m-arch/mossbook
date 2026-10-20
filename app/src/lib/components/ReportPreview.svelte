<script lang="ts">
	// HTML view of the same document model that becomes the PDF.
	import type { ReportDoc } from '#lib/api/index.js';

	let { doc }: { doc: ReportDoc } = $props();
</script>

<article class="paper" aria-label={doc.title}>
	<h2 class="title">{doc.title}</h2>
	<p class="subtitle">{doc.subtitle}</p>
	{#each doc.summary as line (line)}<p class="summary">{line}</p>{/each}
	{#each doc.sections as section (section.heading)}
		<section>
			<h3>{section.heading}</h3>
			{#each section.items as item, i (i)}
				<div class="item">
					<p class="item-title">{item.title}</p>
					{#if item.meta}<p class="meta">{item.meta}</p>{/if}
					{#each item.fields as field, j (j)}
						<p class="label">{field.label}</p>
						<p class="value">{field.value}</p>
					{/each}
				</div>
			{/each}
		</section>
	{/each}
	<p class="footer">{doc.footer}</p>
</article>

<style>
	.paper {
		padding: 1.5rem 1.25rem;
		color: #1c211c;
		background: #fff;
		border-radius: var(--radius-sm);
		box-shadow: var(--shadow-raised);
		font-size: 0.85rem;
		line-height: 1.4;
	}
	p {
		margin: 0;
	}
	.title {
		font-size: 1.25rem;
	}
	.subtitle,
	.meta,
	.label,
	.footer {
		color: #63695f;
	}
	.subtitle {
		margin-bottom: 0.5rem;
	}
	h3 {
		margin: 1.1rem 0 0.25rem;
		font-size: 0.95rem;
	}
	.item {
		padding: 0.5rem 0;
		border-top: 1px solid #dadbd5;
	}
	.item-title {
		font-weight: 650;
	}
	.label {
		margin-top: 0.3rem;
		font-size: 0.72rem;
		font-weight: 650;
	}
	.value {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.footer {
		margin-top: 1rem;
		font-size: 0.7rem;
	}
</style>

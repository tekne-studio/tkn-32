<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, aside, children }: { title: string; aside?: Snippet; children: Snippet } = $props();
</script>

<section>
	<header>
		<h2>{title}</h2>
		{#if aside}<div class="aside">{@render aside()}</div>{/if}
	</header>
	{@render children()}
</section>

<style>
	section {
		background: var(--panel);
		border: 1px solid var(--rule);
		border-radius: var(--radius);
		padding: 10px;
		min-width: 0;
	}
	header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--gap);
		margin-bottom: 8px;
	}
	h2 {
		margin: 0;
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--dim);
	}
	.aside {
		color: var(--dim);
		font-size: 12px;
		text-align: right;
	}
</style>

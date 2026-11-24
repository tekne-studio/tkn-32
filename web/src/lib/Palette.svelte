<script lang="ts">
	let { palette, lit }: { palette: number[]; lit: number | null } = $props();

	const hex = (n: number) => n.toString(16).padStart(2, '0');
	let colours = $derived(
		Array.from({ length: palette.length / 4 }, (_, i) => {
			const [r, g, b, a] = palette.slice(i * 4, i * 4 + 4);
			return { css: `rgb(${r} ${g} ${b} / ${a / 255})`, hex: `${hex(r)}${hex(g)}${hex(b)}${a === 255 ? '' : hex(a)}` };
		})
	);
</script>

<ol>
	{#each colours as colour, i (i)}
		<li class:lit={lit === i} title="#{colour.hex}">
			<span class="swatch" style:background={colour.css}></span>
			<!-- One digit of base 32, as an agent reads a screen. -->
			<span class="index">{i.toString(32)}</span>
			<span class="hex">{colour.hex}</span>
		</li>
	{/each}
</ol>

<style>
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(8, minmax(0, 1fr));
		gap: 4px;
	}
	li {
		border: 1px solid transparent;
		border-radius: 4px;
		padding: 2px;
		display: grid;
		grid-template-columns: auto 1fr;
		column-gap: 4px;
		align-items: center;
		min-width: 0;
	}
	li.lit {
		border-color: var(--accent);
	}
	.swatch {
		grid-column: 1 / -1;
		height: 22px;
		border-radius: 3px;
		box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
		margin-bottom: 2px;
	}
	.index {
		color: var(--ink);
	}
	.hex {
		color: var(--dim);
		font-size: 10px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>

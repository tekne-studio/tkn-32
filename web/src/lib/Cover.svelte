<script lang="ts">
	import type { Covered } from './bench';

	/** The cover read from the file: its text reaches the page as text, never as markup. */
	let { cover, width, height }: { cover: Covered; width: number; height: number } = $props();

	let canvas: HTMLCanvasElement;

	$effect(() => {
		const pixels = new ImageData(new Uint8ClampedArray(cover.rgba), width, height);
		canvas.getContext('2d')!.putImageData(pixels, 0, 0);
	});
</script>

<div class="cover">
	<canvas bind:this={canvas} {width} {height}></canvas>
	{#if cover.said}
		<dl>
			<dt>title</dt>
			<dd>{cover.said.title}</dd>
			<dt>maker</dt>
			<dd>{cover.said.maker}</dd>
			<dt>year</dt>
			<dd>{cover.said.year}</dd>
		</dl>
	{:else}
		<p class="hint">
			This file carries no cover: a cabinet shows the start of its name, as here. A <code>cover.png</code> and
			<code>cartridge_build::cover</code> in the cart's <code>build.rs</code> give it one.
		</p>
	{/if}
</div>

<style>
	.cover {
		contain: inline-size;
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		align-items: start;
	}
	canvas {
		width: 320px;
		max-width: 100%;
		image-rendering: pixelated;
		border-radius: 4px;
	}
	dl {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 4px 10px;
		margin: 0;
	}
	dt {
		color: var(--dim);
	}
	dd {
		margin: 0;
	}
	.hint {
		flex: 1 1 200px;
		margin: 0;
		color: var(--dim);
		font-size: 12px;
	}
</style>

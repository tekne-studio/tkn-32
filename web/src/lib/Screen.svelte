<script lang="ts">
	import { HEIGHT, WIDTH } from './bench';

	let {
		rgba,
		indices,
		colours,
		scale,
		pointed = $bindable()
	}: {
		rgba: ArrayBuffer | undefined;
		indices: ArrayBuffer | undefined;
		/** How many colours the palette holds: a pixel's index names one of them. */
		colours: number;
		scale: number;
		/** The pixel under the pointer: where it is and the colour it names. */
		pointed?: { x: number; y: number; index: number } | null;
	} = $props();

	let canvas: HTMLCanvasElement;
	let at: { x: number; y: number } | null = $state(null);

	$effect(() => {
		if (!rgba) return;
		const pixels = new ImageData(new Uint8ClampedArray(rgba.slice(0)), WIDTH, HEIGHT);
		canvas.getContext('2d')!.putImageData(pixels, 0, 0);
	});

	$effect(() => {
		pointed = at && indices ? { ...at, index: new Uint8Array(indices)[at.y * WIDTH + at.x] % colours } : null;
	});

	function point(event: PointerEvent) {
		const box = canvas.getBoundingClientRect();
		const x = Math.floor(((event.clientX - box.left) / box.width) * WIDTH);
		const y = Math.floor(((event.clientY - box.top) / box.height) * HEIGHT);
		at = x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT ? { x, y } : null;
	}
</script>

<canvas
	bind:this={canvas}
	width={WIDTH}
	height={HEIGHT}
	style:width="{WIDTH * scale}px"
	style:height="{HEIGHT * scale}px"
	onpointermove={point}
	onpointerleave={() => (at = null)}
></canvas>

<style>
	canvas {
		max-width: 100%;
		height: auto !important;
		aspect-ratio: 4 / 3;
		background: #000;
		cursor: crosshair;
	}
</style>

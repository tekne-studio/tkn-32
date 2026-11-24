<script lang="ts">
	let {
		width,
		height,
		rgba,
		indices,
		colours,
		scale,
		pointed = $bindable()
	}: {
		/** The console's screen, in pixels. */
		width: number;
		height: number;
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
		const pixels = new ImageData(new Uint8ClampedArray(rgba.slice(0)), width, height);
		canvas.getContext('2d')!.putImageData(pixels, 0, 0);
	});

	$effect(() => {
		pointed = at && indices ? { ...at, index: new Uint8Array(indices)[at.y * width + at.x] % colours } : null;
	});

	function point(event: PointerEvent) {
		const box = canvas.getBoundingClientRect();
		const x = Math.floor(((event.clientX - box.left) / box.width) * width);
		const y = Math.floor(((event.clientY - box.top) / box.height) * height);
		at = x >= 0 && x < width && y >= 0 && y < height ? { x, y } : null;
	}
</script>

<canvas
	bind:this={canvas}
	width={width}
	height={height}
	style:width="{width * scale}px"
	style:height="{height * scale}px"
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

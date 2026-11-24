<script lang="ts">
	import { KEPT } from './bench.svelte';

	let { spent, budget }: { spent: number[]; budget: number } = $props();

	const HIGH = 64;
	let canvas: HTMLCanvasElement;
	/** The strip's top: a quarter of the budget, the line a cart's test holds it under, with room above. */
	let top = $derived(Math.max(budget / 3, ...spent));

	$effect(() => {
		const context = canvas.getContext('2d')!;
		const style = getComputedStyle(canvas);
		const y = (units: number) => HIGH - Math.round((units / top) * HIGH);
		context.clearRect(0, 0, KEPT, HIGH);
		context.fillStyle = style.getPropertyValue('--faint');
		context.fillRect(0, y(budget / 4), KEPT, 1);
		const from = KEPT - spent.length;
		spent.forEach((units, i) => {
			context.fillStyle = style.getPropertyValue(units > budget / 4 ? '--alert' : '--accent');
			context.fillRect(from + i, y(units), 1, HIGH - y(units));
		});
	});

	let last = $derived(spent.at(-1) ?? 0);
	let mean = $derived(spent.length ? Math.round(spent.reduce((a, b) => a + b, 0) / spent.length) : 0);
	let peak = $derived(Math.max(0, ...spent));
	const share = (units: number) => `${((units / budget) * 100).toFixed(2)}%`;
</script>

<canvas bind:this={canvas} width={KEPT} height={HIGH}></canvas>
<dl>
	<div><dt>last</dt><dd>{last.toLocaleString('en')} <small>{share(last)}</small></dd></div>
	<div><dt>mean</dt><dd>{mean.toLocaleString('en')}</dd></div>
	<div><dt>peak</dt><dd>{peak.toLocaleString('en')} <small>{share(peak)}</small></dd></div>
	<div><dt>budget</dt><dd>{budget.toLocaleString('en')}</dd></div>
</dl>

<style>
	canvas {
		width: 100%;
		height: 64px;
		image-rendering: auto;
		background: var(--ground);
		border-radius: 4px;
	}
	dl {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: var(--gap);
		margin: 8px 0 0;
	}
	dt {
		color: var(--dim);
		font-size: 11px;
	}
	dd {
		margin: 0;
	}
	small {
		color: var(--dim);
	}
</style>

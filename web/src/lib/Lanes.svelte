<script lang="ts">
	import { KEPT } from './bench.svelte';
	import { CHANNELS, PITCHES, WAVES, line, silences, type Played, type Tone } from './tone';

	let {
		played,
		now,
		ticks,
		onpick
	}: {
		/** The tones of the last [`KEPT`] updates, in order. */
		played: Played[];
		/** The updates run so far. */
		now: number;
		/** Updates a second: how long a tone lasts, in updates. */
		ticks: number;
		/** A tone clicked, for the editor. */
		onpick: (tone: Tone) => void;
	} = $props();

	const ROW = 40;
	const HIGH = ROW * CHANNELS;
	const COLOURS = ['--accent', '--warm', '--violet', '--dim'];
	let canvas: HTMLCanvasElement;
	let pointed: Tone | null = $state(null);

	/** Where each tone lies: from its update to its length's end or the next tone on its channel. */
	let spans = $derived.by(() => {
		const spans: { from: number; to: number; tone: Tone }[] = [];
		const last: ({ to: number } | undefined)[] = [];
		for (const { update, tone } of played) {
			const before = last[tone.channel];
			if (before && before.to > update) before.to = update;
			if (silences(tone)) {
				last[tone.channel] = undefined;
				continue;
			}
			const span = { from: update, to: update + Math.max(1, (tone.ms * ticks) / 1000), tone };
			spans.push(span);
			last[tone.channel] = span;
		}
		return spans;
	});

	/** A pitch's height in its channel's row: equal ratios, equal steps. */
	const [low, high] = PITCHES.map(Math.log);
	const height = (hz: number) => (Math.log(Math.min(Math.max(hz, PITCHES[0]), PITCHES[1])) - low) / (high - low);
	const x = (update: number) => update - (now - KEPT);

	$effect(() => {
		const context = canvas.getContext('2d')!;
		const style = getComputedStyle(canvas);
		context.clearRect(0, 0, KEPT, HIGH);
		for (let channel = 0; channel < CHANNELS; channel++) {
			context.fillStyle = style.getPropertyValue('--rule');
			context.fillRect(0, channel * ROW + ROW - 1, KEPT, 1);
		}
		context.lineWidth = 2;
		for (const { from, to, tone } of spans) {
			if (x(to) < 0) continue;
			const top = tone.channel * ROW + 2;
			const y = (hz: number) => top + (ROW - 6) * (1 - height(hz));
			const length = Math.max(1, (tone.ms * ticks) / 1000);
			// The pitch it reaches where it is cut, sliding by equal ratios.
			const reached = tone.hz * Math.pow(tone.toHz / tone.hz, Math.min(1, (to - from) / length));
			const gradient = context.createLinearGradient(x(from), 0, x(to), 0);
			const colour = style.getPropertyValue(COLOURS[tone.wave] ?? '--ink');
			const at = (loud: number) => `color-mix(in srgb, ${colour} ${Math.round(25 + (loud / 255) * 75)}%, transparent)`;
			gradient.addColorStop(0, at(tone.loud));
			gradient.addColorStop(1, at(tone.loud + (tone.toLoud - tone.loud) * Math.min(1, (to - from) / length)));
			context.strokeStyle = gradient;
			context.setLineDash(tone.wave === 3 ? [2, 2] : []);
			context.beginPath();
			context.moveTo(x(from), y(tone.hz));
			context.lineTo(Math.max(x(to), x(from) + 1), y(reached));
			context.stroke();
		}
		context.setLineDash([]);
	});

	function at(event: MouseEvent): Tone | null {
		const box = canvas.getBoundingClientRect();
		const update = (event.clientX - box.left) * (KEPT / box.width) + (now - KEPT);
		const channel = Math.floor(((event.clientY - box.top) * (HIGH / box.height)) / ROW);
		const found = spans.findLast((span) => span.tone.channel === channel && span.from <= update + 1 && update - 1 <= span.to);
		return found?.tone ?? null;
	}
</script>

<div class="lanes">
	<ol aria-hidden="true">
		{#each Array.from({ length: CHANNELS }, (_, i) => i) as channel (channel)}<li>{channel}</li>{/each}
	</ol>
	<canvas
	bind:this={canvas}
	width={KEPT}
	height={HIGH}
	onpointermove={(event) => (pointed = at(event))}
	onpointerleave={() => (pointed = null)}
	onclick={(event) => {
		const tone = at(event);
		if (tone) onpick(tone);
	}}
	class:pointing={pointed}
	></canvas>
</div>
<p class="legend">
	{#if pointed}
		<code>{line(pointed)}</code> · click to edit
	{:else}
		{#each WAVES as wave, i (wave)}<span style:color="var({COLOURS[i]})">{wave.toLowerCase()}</span>{' '}{/each}
		· a line a tone, its height the pitch, its strength the loudness
	{/if}
</p>

<style>
	.lanes {
		/* The screen sets the column's width; the lanes fill it. */
		contain: inline-size;
		position: relative;
	}
	ol {
		position: absolute;
		inset: 0 auto 0 0;
		display: grid;
		grid-template-rows: repeat(4, 1fr);
		margin: 0;
		padding: 2px 0 0 4px;
		list-style: none;
		color: var(--faint);
		font-size: 10px;
		pointer-events: none;
	}
	canvas {
		display: block;
		width: 100%;
		height: 160px;
		background: var(--ground);
		border-radius: 4px;
	}
	.pointing {
		cursor: pointer;
	}
	.legend {
		/* Wraps to the column instead of widening it. */
		contain: inline-size;
		margin: 6px 0 0;
		color: var(--dim);
		font-size: 12px;
		min-height: 1.4em;
	}
	code {
		color: var(--ink);
	}
</style>

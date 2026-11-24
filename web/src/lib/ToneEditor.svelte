<script lang="ts">
	import { CHANNELS, LENGTHS, LOUDEST, PITCHES, WAVES, line, parse, type Tone } from './tone';

	let {
		tone = $bindable(),
		onplay
	}: {
		tone: Tone;
		onplay: (tone: Tone) => void;
	} = $props();

	/** A pitch slider moves by equal ratios: its 0..1000 spans the chip's pitches. */
	const STEPS = 1000;
	const [low, high] = PITCHES.map(Math.log);
	const toSlider = (hz: number) => Math.round(((Math.log(Math.max(hz, PITCHES[0])) - low) / (high - low)) * STEPS);
	const toHz = (at: number) => Math.round(Math.exp(low + (at / STEPS) * (high - low)));

	let copied = $state(false);
	/** Whether the line typed is no tone. */
	let wrong = $state(false);

	/** A line typed or pasted: the tone it says, played, when it says one. */
	function typed(text: string) {
		const tone = parse(text);
		wrong = !tone;
		if (tone) changed(tone);
	}
	async function copy() {
		await navigator.clipboard.writeText(line(tone));
		copied = true;
		setTimeout(() => (copied = false), 1200);
	}

	/** Plays the tone as it now is: each change is heard, as a knob is. */
	function changed(change: Partial<Tone>) {
		tone = { ...tone, ...change };
		onplay(tone);
	}
</script>

<div class="editor">
	<label>
		channel
		<select value={tone.channel} onchange={(event) => changed({ channel: +event.currentTarget.value })}>
			{#each Array.from({ length: CHANNELS }, (_, i) => i) as channel (channel)}<option value={channel}>{channel}</option>{/each}
		</select>
	</label>
	<label>
		wave
		<select value={tone.wave} onchange={(event) => changed({ wave: +event.currentTarget.value })}>
			{#each WAVES as wave, i (wave)}<option value={i}>{wave.toLowerCase()}</option>{/each}
		</select>
	</label>
	<label>
		ms
		<input type="number" min={LENGTHS[0]} max={LENGTHS[1]} value={tone.ms} onchange={(event) => changed({ ms: +event.currentTarget.value })} />
	</label>
	<button onclick={() => onplay(tone)}>Play</button>

	<span class="row">
		<span class="name">hz</span>
		<input type="range" min="0" max={STEPS} value={toSlider(tone.hz)} oninput={(event) => changed({ hz: toHz(+event.currentTarget.value) })} />
		<input type="number" min={PITCHES[0]} max={PITCHES[1]} value={tone.hz} onchange={(event) => changed({ hz: +event.currentTarget.value })} />
	</span>
	<span class="row">
		<span class="name">to hz</span>
		<input type="range" min="0" max={STEPS} value={toSlider(tone.toHz)} oninput={(event) => changed({ toHz: toHz(+event.currentTarget.value) })} />
		<input type="number" min={PITCHES[0]} max={PITCHES[1]} value={tone.toHz} onchange={(event) => changed({ toHz: +event.currentTarget.value })} />
	</span>
	<span class="row">
		<span class="name">loud</span>
		<input type="range" min="0" max={LOUDEST} value={tone.loud} oninput={(event) => changed({ loud: +event.currentTarget.value })} />
		<input type="number" min="0" max={LOUDEST} value={tone.loud} onchange={(event) => changed({ loud: +event.currentTarget.value })} />
	</span>
	<span class="row">
		<span class="name">to loud</span>
		<input type="range" min="0" max={LOUDEST} value={tone.toLoud} oninput={(event) => changed({ toLoud: +event.currentTarget.value })} />
		<input type="number" min="0" max={LOUDEST} value={tone.toLoud} onchange={(event) => changed({ toLoud: +event.currentTarget.value })} />
	</span>

	<span class="line">
		<input
			class="code"
			class:wrong
			value={line(tone)}
			spellcheck="false"
			aria-label="The line a game writes; type or paste one and press Enter"
			onchange={(event) => typed(event.currentTarget.value)}
			oninput={() => (wrong = false)}
		/>
		<button onclick={copy}>{copied ? 'Copied' : 'Copy'}</button>
	</span>
</div>

<style>
	.editor {
		/* As wide as its column, never wider. */
		contain: inline-size;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 12px;
		margin-top: 10px;
	}
	label {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--dim);
	}
	input[type='number'] {
		width: 7ch;
	}
	.row {
		display: grid;
		grid-template-columns: 7ch minmax(0, 1fr) 8ch;
		align-items: center;
		gap: 8px;
		flex: 1 1 100%;
	}
	.name {
		color: var(--dim);
	}
	input[type='range'] {
		accent-color: var(--accent);
		padding: 0;
		border: 0;
		background: none;
	}
	.line {
		display: flex;
		align-items: center;
		gap: 8px;
		flex: 1 1 100%;
		contain: inline-size;
	}
	.code {
		flex: 1;
		min-width: 0;
		padding: 6px 8px;
		background: var(--ground);
		border-color: transparent;
	}
	.code.wrong {
		border-color: var(--alert);
	}
</style>

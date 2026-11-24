// The arcade's chip in an AudioWorklet: it runs the chip's module (this
// crate, `crates/chip-worklet`, given as bytes when the node is made) and
// plays the tones a page sends, each update's at its own time, into the
// node the page connects it to. Every page that plays a cart sounds it
// through this, the workbench among them.
//
// A page sends the tones of the updates it ran as one Uint32Array,
// transferred (the workbench's `sound.svelte.ts` writes one, `batch`): first
// how many updates a second the game runs, 0 to play every update at once,
// then for each update how many tones it played and their words (`env.tone`'s
// seven a tone), an update that played nothing among them. The worklet lays
// them one update apart from a cursor kept a little ahead of what it renders,
// so tones from one batch spread out as the game played them, and the next
// batch goes on from where the last one ended. `{ hush: true }` drops what
// waits and lets every channel go out; `{ end: true }` hushes it for good,
// and the processor ends once the channels are out, so a node let go leaves
// nothing running.
//
// What waits is a ring made once, in the order it plays: the cursor only
// moves forward, and is set back to the lead only when everything before
// it is due. A page whose clock runs ahead of the audio device's, or the
// batches a node made late takes at once, push the cursor ahead: a batch
// that begins past `SLACK` lays as many updates that played nothing in no
// time, a silence shortened, and past `AHEAD` no update takes time, so the
// latency comes back and no tone waits behind another.
// Plain JavaScript, served as it is: a worklet's scope loads it alone, and
// its globals (`sampleRate`, `currentFrame`, `registerProcessor`) are in no
// page's types, so the pages that serve it leave it unchecked.
// @ts-nocheck

/**
 * How far ahead of the rendering the cursor is set, in seconds: the room a
 * batch may come late, by a frame's jitter, and still play each update one
 * step after the last.
 */
const LEAD = 0.03;
/**
 * How far past the lead a batch may begin before its silences are
 * shortened: an update and a frame's jitter, never reached by a page whose
 * clock keeps with the device's.
 */
const SLACK = 0.04;
/** How far past the lead the cursor may run at all. */
const AHEAD = 0.25;
/** How long an ended chip renders on, in seconds: its channels' way out. */
const TAIL = 0.05;
const CHANNELS = 4;
const WORDS = 7;
/**
 * The most tones that wait: `AHEAD` and a batch of updates at the most
 * tones an update plays, 16, with room. Past it a tone is let go.
 */
const RING = 512;

class ChipProcessor extends AudioWorkletProcessor {
	constructor(options) {
		super();
		const module = new WebAssembly.Module(options.processorOptions.module);
		this.chip = new WebAssembly.Instance(module, {}).exports;
		this.chip.start(sampleRate);
		/** Each waiting tone's frame and words, from `head`, `count` of them. */
		this.at = new Float64Array(RING);
		this.words = new Uint32Array(RING * WORDS);
		this.head = 0;
		this.count = 0;
		/** The chip's rendered samples, a view made again only when its memory moves. */
		this.samples = null;
		/** The frame the next update's tones play at. */
		this.cursor = 0;
		/** The frame it ends at, once told to. */
		this.ends = Infinity;
		this.port.onmessage = (event) => {
			const data = event.data;
			if (data instanceof Uint32Array) {
				if (this.ends === Infinity) this.take(data);
			} else if (data.end) {
				this.hush();
				this.ends = currentFrame + TAIL * sampleRate;
			} else if (data.hush) this.hush();
		};
	}

	take(played) {
		const step = played[0] > 0 ? sampleRate / played[0] : 0;
		const lead = currentFrame + LEAD * sampleRate;
		// Came later than the lead: everything waiting is due, so the
		// cursor starts again from the lead, and the queue stays in order.
		if (this.cursor < currentFrame) this.cursor = lead;
		// How far this batch begins past the slack: so much silence it may
		// take out.
		let over = this.cursor - lead - SLACK * sampleRate;
		const ahead = lead + AHEAD * sampleRate;
		for (let at = 1; at < played.length; ) {
			const tones = played[at++];
			for (let tone = 0; tone < tones && at + WORDS <= played.length; tone++, at += WORDS) {
				this.push(Math.round(this.cursor), played, at);
			}
			if (tones === 0 && over > 0) over -= step;
			else if (tones === 0 || this.cursor <= ahead) this.cursor += step;
		}
	}

	/** One tone into the ring, at its frame; let go when the ring is full. */
	push(frame, played, from) {
		if (this.count === RING) return;
		const slot = (this.head + this.count) % RING;
		this.at[slot] = frame;
		for (let word = 0; word < WORDS; word++) this.words[slot * WORDS + word] = played[from + word];
		this.count++;
	}

	hush() {
		this.head = 0;
		this.count = 0;
		this.cursor = 0;
		for (let channel = 0; channel < CHANNELS; channel++) this.chip.play(channel, 0, 0, 0, 0, 0, 0);
	}

	// Allocates nothing: the audio thread must never wait on a collection.
	process(_, outputs) {
		const out = outputs[0];
		const left = out[0];
		const block = left.length;
		const words = this.words;
		let done = 0;
		while (done < block) {
			const frame = currentFrame + done;
			while (this.count > 0 && this.at[this.head] <= frame) {
				const w = this.head * WORDS;
				this.chip.play(words[w], words[w + 1], words[w + 2], words[w + 3], words[w + 4], words[w + 5], words[w + 6]);
				this.head = (this.head + 1) % RING;
				this.count--;
			}
			const due = this.count > 0 ? this.at[this.head] - currentFrame : block;
			const len = Math.max(1, Math.min(block, due) - done);
			const from = this.chip.render(len);
			const buffer = this.chip.memory.buffer;
			if (this.samples?.buffer !== buffer || this.samples.byteOffset !== from) {
				this.samples = new Float32Array(buffer, from, block);
			}
			for (let i = 0; i < len; i++) left[done + i] = this.samples[i];
			done += len;
		}
		for (let channel = 1; channel < out.length; channel++) out[channel].set(out[0]);
		return currentFrame + block < this.ends;
	}
}

registerProcessor('chip', ChipProcessor);

// The worklet plays each tone on the sample the game played it, as a page
// feeds it: every golden session of the console (`crates/headless/tests/
// goldens.rs`, which keeps what each played beside its cart, one batch from
// the emulator's encoder) sent in the batches a page's frames make, each
// cut from it at the updates' bounds, slow and fast, late and jittered, rendered
// in the audio thread's blocks on a clock of its own, gives every sample
// the headless runner gives, at 48 kHz to its kept hash and at 44.1 kHz
// to the chip rendered there update by update. Held and resumed as a
// cabinet holds its game, the batch of the update before the hold coming
// after it, it gives the same samples, the stream eased out and in around
// the hold and silent while it holds; ended as it eases out, it eases on
// to silence, and ended, a hold keeps it no more. It plays the files a page
// loads, the chip and the processor of the console's build of the emulator
// (`bun scripts/emulator.ts`, which `bun scripts/check.ts` runs first).
import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { EMULATOR } from '../../scripts/emulator';

const ROOT = `${import.meta.dir}/../..`;
const MANIFEST = `${EMULATOR}/emulator.json`;
if (!existsSync(MANIFEST)) throw new Error(`no emulator built in ${EMULATOR}: bun scripts/emulator.ts`);
/** The emulator's chip and processor, by the names its manifest gives them. */
const { chip, processor }: { chip: string; processor: string } = JSON.parse(readFileSync(MANIFEST, 'utf8'));
const KEPT = `${ROOT}/crates/headless/tests/kept`;
const GOLDENS = ['rocks', 'dot', 'boom', 'hues'];
/** The rate the headless runner hears at, and its goldens are hashed at. */
const HEADLESS = 48_000;
const UPDATES_A_SECOND = 60;
/** The most updates the shelf runs in a page's frame (`CATCH_UP`). */
const CATCH_UP = 4;
const BLOCK = 128;
const WORDS = 7;
/** Never further ahead than this, in seconds: the lead, the slack and a batch of four updates. */
const FAR = 0.03 + 0.04 + CATCH_UP / UPDATES_A_SECOND;

interface Kept {
	samples: string;
	length: number;
	/** Its rate, then each update's count of tones and their words. */
	batch: number[];
}

/** A session's tones, each update's part of the kept batch, its count first. */
interface Played {
	samples: string;
	length: number;
	rate: number;
	updates: number[][];
}

interface Chip {
	memory: WebAssembly.Memory;
	start(rate: number): void;
	play(...words: number[]): void;
	render(len: number): number;
}

interface Processor {
	port: { onmessage(event: { data: unknown }): void };
	process(inputs: unknown[], outputs: Float32Array[][]): boolean;
	chip: Chip;
	/** Its own time: the frames it rendered. */
	now: number;
	/** The ring: each waiting tone's frame, from `head`, `count` of them. */
	at: Float64Array;
	head: number;
	count: number;
}

/** A page as it draws: its rate, how much a frame wanders, a frame late now and then. */
interface Page {
	hz: number;
	jitterMs: number;
	lateEvery?: number;
	lateMs?: number;
	/** How far the audio clock runs from the page's, parts a million. */
	ppm?: number;
}

const PAGES: Record<string, Page> = {
	'a page at 60 Hz, jittered': { hz: 60, jitterMs: 4 },
	'a slow page, 30 Hz': { hz: 30, jitterMs: 6 },
	'a fast page, 120 Hz': { hz: 120, jitterMs: 2 },
	'a frame 20 ms late every second': { hz: 60, jitterMs: 2, lateEvery: 60, lateMs: 20 },
	'an audio clock 200 ppm off': { hz: 60, jitterMs: 2, ppm: 200 },
};

const scope = globalThis as unknown as Record<string, unknown>;
let Worklet: (new (options: { processorOptions: { module: Uint8Array } }) => Processor) | null = null;
scope.AudioWorkletProcessor = class {
	port = { onmessage: (_: { data: unknown }) => {} };
};
scope.registerProcessor = (_: string, processor: typeof Worklet) => (Worklet = processor);
scope.sampleRate = HEADLESS;
await import(`${EMULATOR}/${processor}`);

function read(name: string): Played {
	const { samples, length, batch }: Kept = JSON.parse(readFileSync(`${KEPT}/${name}.played.json`, 'utf8'));
	const updates: number[][] = [];
	for (let at = 1; at < batch.length; ) {
		const end = at + 1 + batch[at] * WORDS;
		updates.push(batch.slice(at, end));
		at = end;
	}
	return { samples, length, rate: batch[0], updates };
}

/** The batch of `updates`, each a part of the kept one, at the kept rate. */
const batch = (played: Played, updates: number[][]) => Uint32Array.from([played.rate, ...updates.flat()]);


/** FNV-1a, 64 bits, over the samples' bytes, as goldens.rs hashes them. */
function fingerprint(samples: Float32Array): string {
	const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
	let low = 0x84222325;
	let high = 0xcbf29ce4;
	for (const byte of bytes) {
		low = (low ^ byte) >>> 0;
		// Times 0x100000001B3: 0x1B3 in full, and 1 << 40, the low half into the high.
		const product = low * 0x1b3;
		high = (high * 0x1b3 + Math.floor(product / 2 ** 32) + ((low << 8) >>> 0)) % 2 ** 32;
		low = product % 2 ** 32;
	}
	return `0x${[high, low].map((half) => half.toString(16).toUpperCase().padStart(8, '0')).join('')}`;
}

/** The chip rendered as the headless runner renders it: an update's tones, then its samples. */
function reference(module: Uint8Array, played: Played, rate: number): Float32Array {
	const chip = new WebAssembly.Instance(new WebAssembly.Module(module), {}).exports as unknown as Chip;
	chip.start(rate);
	const out = new Float32Array((played.length * rate) / HEADLESS);
	let done = 0;
	const render = (samples: number) => {
		for (const end = Math.min(done + samples, out.length); done < end; ) {
			const len = Math.min(BLOCK, end - done);
			out.set(new Float32Array(chip.memory.buffer, chip.render(len), len), done);
			done += len;
		}
	};
	for (const update of played.updates) {
		for (let at = 1; at < update.length; at += WORDS) chip.play(...update.slice(at, at + WORDS));
		render(rate / UPDATES_A_SECOND);
	}
	render(out.length - done);
	return out;
}

/**
 * A hold, as a cabinet's: from the first block at `atMs` a batch comes in,
 * for `blocks` of them. The batch comes after the hold, as the one of the
 * update a game ran before it does, its page told of the hold first.
 */
interface Hold {
	atMs: number;
	blocks: number;
}

/** What a worklet rendered, by the audio thread's frames, and where it held and resumed. */
interface Heard {
	out: Float32Array;
	held: number;
	resumed: number;
}

/**
 * The worklet fed as a page feeds it: each frame, every update the game ran
 * since the last (at most `CATCH_UP`), posted 20 ms before the audio thread
 * takes it, and rendered in blocks on the audio thread's own clock. Held,
 * the game and its page's batches wait for it: they come by the worklet's
 * own time, which a hold stops.
 */
function worklet(module: Uint8Array, played: Played, rate: number, page: Page, hold?: Hold): Heard {
	scope.sampleRate = rate;
	const processor = new Worklet!({ processorOptions: { module } });
	let seed = 1;
	const random = () => (seed = (Math.imul(seed, 1103515245) + 12345) >>> 0) / 2 ** 32;
	const batches: { atMs: number; played: Uint32Array }[] = [];
	let ms = 0;
	let ran = 0;
	for (let frame = 1; ran < played.updates.length; frame++) {
		const late = page.lateEvery && frame % page.lateEvery === 0 ? (page.lateMs ?? 0) : 0;
		ms += 1000 / page.hz + (random() * 2 - 1) * page.jitterMs + late;
		const owed = Math.min(Math.floor((ms * UPDATES_A_SECOND) / 1000), played.updates.length) - ran;
		const count = Math.min(owed, CATCH_UP);
		if (count <= 0) continue;
		batches.push({ atMs: ms + 20, played: batch(played, played.updates.slice(ran, ran + count)) });
		ran += owed;
	}
	const holds = hold ? hold.blocks * BLOCK : 0;
	const out = new Float32Array(Math.ceil(((ms / 1000) * rate) + played.length * (rate / HEADLESS)) + rate + holds);
	const heard: Heard = { out, held: -1, resumed: -1 };
	let next = 0;
	for (let frame = 0; frame + BLOCK <= out.length; frame += BLOCK) {
		const nowMs = (processor.now / rate) * 1000 * (1 + (page.ppm ?? 0) / 1e6);
		const batchNow = next < batches.length && batches[next].atMs <= nowMs;
		if (hold && heard.held < 0 && (frame / rate) * 1000 >= hold.atMs && batchNow) {
			processor.port.onmessage({ data: { hold: true } });
			heard.held = frame;
		} else if (heard.held >= 0 && heard.resumed < 0 && frame === heard.held + holds) {
			processor.port.onmessage({ data: { resume: true } });
			heard.resumed = frame;
		}
		for (; next < batches.length && batches[next].atMs <= nowMs; next++) {
			processor.port.onmessage({ data: batches[next].played });
		}
		const block = new Float32Array(BLOCK);
		processor.process([], [[block]]);
		out.set(block, frame);
	}
	return heard;
}

/**
 * The samples a stream held at `held` and resumed at `resumed` plays, by
 * its own time, from the stream never held: eased out over `ease` frames,
 * each before its step down, and in over as many, each after its step up,
 * the chip's sample times the ease, as the worklet reckons it.
 */
function eased(never: Float32Array, held: number, ease: number): Float32Array {
	return never.map((sample, i) => {
		if (i >= held && i < held + ease) return Math.fround((sample * (ease - (i - held))) / ease);
		if (i >= held + ease && i < held + 2 * ease) return Math.fround((sample * (i - held - ease + 1)) / ease);
		return sample;
	});
}

/** What an hour of a page did to the worklet. */
interface Hour {
	/** Tones sent, and played by the chip. */
	sent: number;
	played: number;
	/** Whether every tone waited in the order it plays. */
	ordered: boolean;
	/** The furthest a tone waited ahead of what was rendering, in seconds. */
	furthest: number;
	/** The same, from the second second on. */
	settled: number;
}

/** How the page's clock and the device's go. */
interface Clocks {
	seconds: number;
	/** How far the audio device runs off the page, parts a million, ahead when positive. */
	ppm: number;
	/** Updates the game ran while its node was made, sent at once as it comes, one a batch. */
	waited?: number;
	/** The update of the session the game is at as its node is made. */
	from?: number;
}

/**
 * A session played over and over, on clocks simulated: a page at 60 Hz,
 * jittered, and an audio device of its own. Nothing is kept of the
 * samples.
 */
function run(module: Uint8Array, played: Played, { seconds, ppm, waited = 0, from = 0 }: Clocks): Hour {
	const rate = HEADLESS;
	scope.sampleRate = rate;
	const processor = new Worklet!({ processorOptions: { module } });
	const chip = processor.chip;
	const ring = processor.at.length;
	let heard = 0;
	processor.chip = {
		memory: chip.memory,
		start: chip.start,
		render: chip.render,
		play: (...words) => {
			heard++;
			chip.play(...words);
		}
	};
	let seed = 1;
	const random = () => (seed = (Math.imul(seed, 1103515245) + 12345) >>> 0) / 2 ** 32;
	const total = UPDATES_A_SECOND * seconds;
	const result: Hour = { sent: 0, played: 0, ordered: true, furthest: 0, settled: 0 };
	const send = (frame: number, ran: number, count: number) => {
		const updates = Array.from({ length: count }, (_, k) => played.updates[(from + ran + k) % played.updates.length]);
		result.sent += updates.reduce((tones, update) => tones + update[0], 0);
		processor.port.onmessage({ data: batch(played, updates) });
		const { at, head, count: waiting } = processor;
		for (let k = 1; k < waiting; k++) {
			if (at[(head + k) % ring] < at[(head + k - 1) % ring]) result.ordered = false;
		}
		if (waiting > 0) {
			const ahead = (at[(head + waiting - 1) % ring] - frame) / rate;
			result.furthest = Math.max(result.furthest, ahead);
			if (frame >= 2 * rate) result.settled = Math.max(result.settled, ahead);
		}
	};
	for (let update = 0; update < waited; update++) send(0, update, 1);
	const startMs = (waited * 1000) / UPDATES_A_SECOND;
	let nextMs = startMs + 1000 / UPDATES_A_SECOND;
	let ran = waited;
	const block = new Float32Array(BLOCK);
	for (let frame = 0; ran < total || processor.count > 0; frame += BLOCK) {
		const nowMs = startMs + (frame / rate) * 1000 * (1 + ppm / 1e6);
		for (; ran < total && nextMs + 20 <= nowMs; nextMs += 1000 / UPDATES_A_SECOND + (random() * 2 - 1) * 2) {
			const owed = Math.min(Math.floor((nextMs * UPDATES_A_SECOND) / 1000), total) - ran;
			const count = Math.min(owed, CATCH_UP);
			if (count <= 0) continue;
			send(frame, ran, count);
			ran += owed;
		}
		processor.process([], [[block]]);
	}
	result.played = heard;
	return result;
}

/** The worklet's samples from where it began to sound, as long as the reference. */
function heard(out: Float32Array, reference: Float32Array): Float32Array {
	const from = out.findIndex((sample) => sample !== 0) - reference.findIndex((sample) => sample !== 0);
	return out.slice(from, from + reference.length);
}

describe('the worklet', () => {
	const module = readFileSync(`${EMULATOR}/${chip}`);

	for (const name of GOLDENS) {
		const played = read(name);
		const at48 = reference(module, played, HEADLESS);
		const at44 = reference(module, played, 44_100);

		test(`${name}: the chip renders the headless runner's samples`, () => {
			expect(fingerprint(at48)).toBe(played.samples);
		});

		for (const [what, page] of Object.entries(PAGES)) {
			test(`${name}, ${what}: every sample, at 48 and 44.1 kHz`, () => {
				expect(fingerprint(heard(worklet(module, played, HEADLESS, page).out, at48))).toBe(played.samples);
				const out = heard(worklet(module, played, 44_100, page).out, at44);
				expect(out.length).toBe(at44.length);
				expect(out.findIndex((sample, i) => sample !== at44[i])).toBe(-1);
			});

			// Held mid play for a cabinet's window, 10 s, as its player
			// leaves: silent from the ease out to the resume, and by its own
			// time the stream never held, eased out and in.
			test(`${name}, ${what}: held and resumed, the stream never held, at 48 and 44.1 kHz`, () => {
				for (const rate of [HEADLESS, 44_100]) {
					const never = worklet(module, played, rate, page).out;
					const ease = Math.round(0.005 * rate);
					// The block of the first sample that sounds past two fifths of the session, or of its first.
					const sounds = (from: number) => never.findIndex((sample, i) => i >= from && sample !== 0);
					const late = sounds((played.length * rate) / HEADLESS / 2.5);
					const at = Math.floor((late >= 0 ? late : sounds(0)) / BLOCK) * BLOCK;
					expect(at).toBeGreaterThan(0);
					const hold = { atMs: (at / rate) * 1000, blocks: Math.round((10 * rate) / BLOCK) };
					const { out, held, resumed } = worklet(module, played, rate, page, hold);
					expect(held).toBeGreaterThanOrEqual(at);
					expect(out.subarray(held + ease, resumed).every((sample) => sample === 0)).toBe(true);
					// By its own time, as long as the stream never held: past it both are silent.
					const own = new Float32Array(never.length);
					own.set(out.subarray(0, held + ease));
					own.set(out.subarray(resumed, resumed + never.length - held - ease), held + ease);
					expect(out.subarray(resumed + never.length - held - ease).every((sample) => sample === 0)).toBe(true);
					const expected = eased(never, held, ease);
					expect(own.findIndex((sample, i) => sample !== expected[i])).toBe(-1);
				}
			});
		}
	}

	test('a stream ended while it holds ends at once', () => {
		const played = read('rocks');
		scope.sampleRate = HEADLESS;
		const processor = new Worklet!({ processorOptions: { module } });
		processor.port.onmessage({ data: batch(played, played.updates.slice(0, 60)) });
		const block = new Float32Array(BLOCK);
		for (let blocks = 0; blocks < 100; blocks++) expect(processor.process([], [[block]])).toBe(true);
		processor.port.onmessage({ data: { hold: true } });
		for (let blocks = 0; blocks < 10; blocks++) processor.process([], [[block]]);
		const now = processor.now;
		processor.process([], [[block]]);
		expect(processor.now).toBe(now);
		processor.port.onmessage({ data: { end: true } });
		expect(processor.process([], [[block]])).toBe(false);
	});

	test('a stream ended as it eases out eases on to silence, and ends there', () => {
		const played = read('rocks');
		scope.sampleRate = HEADLESS;
		const processor = new Worklet!({ processorOptions: { module } });
		processor.port.onmessage({ data: batch(played, played.updates.slice(0, 60)) });
		const block = new Float32Array(BLOCK);
		for (let blocks = 0; blocks < 100; blocks++) processor.process([], [[block]]);
		const held = processor.now;
		processor.port.onmessage({ data: { hold: true } });
		processor.process([], [[block]]);
		processor.port.onmessage({ data: { end: true } });
		let blocks = 0;
		while (processor.process([], [[block]]) && blocks < 1000) blocks++;
		expect(blocks).toBeLessThan(2);
		expect(processor.now).toBe(held + Math.round(0.005 * HEADLESS));
	});

	test('a stream ended, then held, ends all the same', () => {
		const played = read('rocks');
		scope.sampleRate = HEADLESS;
		const processor = new Worklet!({ processorOptions: { module } });
		processor.port.onmessage({ data: batch(played, played.updates.slice(0, 60)) });
		const block = new Float32Array(BLOCK);
		for (let blocks = 0; blocks < 100; blocks++) processor.process([], [[block]]);
		processor.port.onmessage({ data: { end: true } });
		processor.port.onmessage({ data: { hold: true } });
		let blocks = 0;
		while (processor.process([], [[block]]) && blocks < 1000) blocks++;
		expect(blocks).toBeLessThanOrEqual(Math.ceil((0.05 * HEADLESS) / BLOCK));
	});

	// A clock 300 ppm off drifts 1.08 s in an hour: the page's ahead, a
	// silence shortened now and then; the device's ahead, the cursor set
	// again from the lead when it falls behind.
	for (const ppm of [300, -300]) {
		test(`an hour of Rocks, the audio clock ${ppm} ppm off: every tone, in order, never far ahead`, () => {
			const { sent, played, ordered, furthest } = run(module, read('rocks'), { seconds: 3600, ppm });
			expect(played).toBe(sent);
			expect(ordered).toBe(true);
			expect(furthest).toBeLessThan(FAR);
		}, 120_000);
	}

	// The most a page keeps for a node on its way, its last 15 batches, all
	// at once, of the 15 updates in a row of Rocks that each play a tone.
	test('a node made late takes what waited, and comes back to the lead', () => {
		const { sent, played, ordered, furthest, settled } = run(module, read('rocks'), { seconds: 15, ppm: 0, waited: 15, from: 76 });
		expect(played).toBe(sent);
		expect(ordered).toBe(true);
		expect(furthest).toBeGreaterThan(FAR);
		expect(settled).toBeLessThan(FAR);
	});
});

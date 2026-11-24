/**
 * The bench as the page drives it: the worker that holds the cartridge,
 * asked each frame to run what the game's clock has due, the keys it holds
 * the buttons by, and what the last answer showed, kept for the panels to
 * read. The clock is the emulator's, as a cabinet's is: the page only sets
 * its pace, the speed while it plays and none while it pauses.
 *
 * Every update's buttons are kept from the start, so the game played so far
 * is a session (`session.ts`): saved, it plays again exactly; and a cart
 * rebuilt is started again and run through it to the update it was at.
 */
import BenchWorker from './bench.worker.ts?worker';
import type { Asked, Console, Covered, Region, Seen, Told } from './bench';
import type { Sound } from './sound.svelte';
import { WORDS, toneOf, type Played } from './tone';

/** How many updates the meter's strip and the sound's lanes keep. */
export const KEPT = 600;
/** The stack a cart built as the guide says keeps below its data, when no map says. */
const STACK = 64 * 1024;

export class Bench {
	/** The console as the emulator says it; null until it is ready. */
	console = $state.raw<Console | null>(null);
	/** Why the emulator could not run. */
	broken = $state('');
	/** What the worker showed last. */
	seen = $state.raw<Seen | undefined>();
	/** What a cabinet shows of the file before anyone plays it. */
	cover = $state.raw<Covered | null>(null);
	/** The name the cartridge was opened by. */
	name = $state('');
	/** What lies where in its memory, by address, when its build mapped it. */
	regions = $state.raw<Region[]>([]);
	seed = $state(1);
	/** The seed the game running was started with. */
	started = $state(1);
	/** Units spent by the last [`KEPT`] updates, oldest first. */
	spent = $state.raw<number[]>([]);
	/** The tones the last [`KEPT`] updates played, in order. */
	played = $state.raw<Played[]>([]);
	/** How warm each block of memory is: 1 the update that changed it, fading. */
	heat = $state.raw<Float32Array>(new Float32Array());
	/** Where the memory shown starts. */
	at = $state(0);
	/** The bytes the memory panel shows. */
	readonly shown = 512;
	/** The buttons the keys hold now. */
	held = $state(0);
	/** Buttons being played back, one an update from the start, until a key takes over. */
	replay = $state.raw<number[] | null>(null);
	/** The update a seek runs to, as fast as the worker goes, before it pauses. */
	seeking = $state<number | null>(null);

	#playing = $state(true);
	/** Updates a second, as a share of the console's. */
	#speed = $state(1);
	/** The buttons of every update run, from the start. */
	#recorded: number[] = [];
	#worker: Worker;
	#file: ArrayBuffer | undefined;
	/** The game begun last: an answer for an older one is dropped. */
	#game = 0;
	/** Whether a tick waits on its answer: the next frame asks no other. */
	#ticking = false;
	#frame = 0;
	/** Whether the memory shown moves to the first block an update writes. */
	#follow = true;

	/** Where the tones of the updates run go; a seek runs silent. */
	readonly sound: Sound;

	constructor(sound: Sound) {
		this.sound = sound;
		this.#worker = new BenchWorker();
		this.#worker.onmessage = ({ data }: MessageEvent<Told>) => this.#told(data);
		this.#frame = requestAnimationFrame(() => this.#tick());
	}

	get playing(): boolean {
		return this.#playing;
	}

	set playing(playing: boolean) {
		this.#playing = playing;
		this.#ask({ pace: this.#pace() });
	}

	get speed(): number {
		return this.#speed;
	}

	set speed(speed: number) {
		this.#speed = speed;
		this.#ask({ pace: this.#pace() });
	}

	/** The updates run since the start: the session's length. */
	get updates(): number {
		return this.seen?.updates ?? 0;
	}

	#pace(): number {
		return this.#playing ? this.#speed : 0;
	}

	#ask(asked: Asked, transfer: Transferable[] = []) {
		this.#worker.postMessage(asked, transfer);
	}

	/** Opens `file` from its start, with the bench's seed. */
	open(file: ArrayBuffer, name: string, regions: Region[] = []) {
		this.#file = file;
		this.name = name;
		this.regions = regions;
		this.restart();
	}

	/**
	 * Starts the cartridge again from its seed: the same seed, the same game.
	 * With `replay`, those buttons are played back from the first update;
	 * with `seek`, run through at once to that update, then paused.
	 */
	restart(replay: number[] | null = null, seek: number | null = null) {
		if (!this.#file) return;
		this.spent = [];
		this.played = [];
		this.heat = new Float32Array();
		this.#recorded = [];
		this.replay = replay;
		this.seeking = seek && seek > 0 ? seek : null;
		this.#follow = this.seeking === null;
		this.sound.hush();
		this.started = this.seed >>> 0;
		const file = this.#file.slice(0);
		this.#ask({ open: file, seed: this.started, game: ++this.#game, script: replay, seek, pace: this.#pace() }, [file]);
	}

	/** Opens a new build of the cartridge and runs it through the game played so far. */
	rebuilt(file: ArrayBuffer, regions: Region[] = []) {
		this.#file = file;
		this.regions = regions;
		this.restart([...this.#recorded], this.updates);
	}

	/** Plays the game played so far again from its start, at the bench's speed. */
	again() {
		this.#playing = true;
		this.restart([...this.#recorded]);
	}

	/** Runs the rest of a replay at once. */
	toEnd() {
		if (!this.replay) return;
		this.seeking = this.replay.length;
		this.#ask({ toEnd: true });
	}

	/** The game played so far: its seed and every update's buttons. */
	session() {
		return { cart: this.name, seed: this.started, buttons: [...this.#recorded], hash: this.seen?.hash };
	}

	/** One update now, with the buttons held; it pauses the clock. */
	step() {
		this.playing = false;
		this.#ask({ step: 1 });
	}

	/** Shows the memory from `at`, held inside it. */
	show(at: number) {
		this.#follow = false;
		const size = this.seen?.size ?? 0;
		const last = Math.max(0, Math.ceil(size / 16) * 16 - this.shown);
		this.at = Math.min(Math.max(0, Math.floor(at / 16) * 16), last);
		this.#ask({ window: { at: this.at, len: this.shown } });
	}

	/**
	 * A key went down: whether it is one of the game's. A key takes over from
	 * a replay as it plays, never from a seek: the emulator ends the script
	 * on a button newly held, so the keys wait for the seek's end.
	 */
	press(code: string): boolean {
		const bit = this.#button(code);
		if (!bit) return false;
		this.held |= bit;
		if (this.seeking !== null) return true;
		this.#ask({ press: this.held });
		if (this.replay) {
			this.replay = null;
			// The script may have run out, and the clock with it, before the key came.
			this.#ask({ pace: this.#pace() });
		}
		return true;
	}

	release(code: string): boolean {
		const bit = this.#button(code);
		if (!bit) return false;
		this.held &= ~bit;
		if (this.seeking === null) this.#ask({ press: this.held });
		return true;
	}

	#button(code: string): number {
		return this.console?.keys.find(([key]) => key === code)?.[1] ?? 0;
	}

	#tick() {
		this.#frame = requestAnimationFrame(() => this.#tick());
		if (this.#ticking || !this.console) return;
		this.#ticking = true;
		this.#ask({ tick: true });
	}

	#told(told: Told) {
		if ('ready' in told) {
			this.console = told.ready;
			return;
		}
		if ('failed' in told) {
			this.broken = told.failed;
			return;
		}
		if (told.ticked) this.#ticking = false;
		// The game restarted under it: its tones, units and updates are not
		// this game's, and the open it waits on is still to come.
		if ('game' in told && told.game === this.#game) this.#took(told);
	}

	#took(seen: Seen) {
		const seeked = this.seeking !== null && seen.seeking === null;
		this.seen = seen;
		this.seeking = seen.seeking;
		if (seen.cover) this.cover = seen.cover;
		if (seen.ran.length) {
			this.#recorded.push(...seen.ran.map(({ buttons }) => buttons));
			this.spent = [...this.spent, ...seen.ran.map(({ spent }) => spent)].slice(-KEPT);
		}
		if (seen.tones.some((words) => words.length)) {
			const first = seen.updates - seen.tones.length;
			const played = this.played.filter(({ update }) => update > seen.updates - KEPT);
			seen.tones.forEach((words, i) => {
				for (let at = 0; at < words.length; at += WORDS) {
					played.push({ update: first + i, tone: toneOf(words.slice(at, at + WORDS)) });
				}
			});
			this.played = played;
		}
		if (seen.sound) this.sound.play(seen.sound);
		// A replay ends at its last update, and the emulator holds the clock there.
		if (this.replay && seen.scripted === null && seen.seeking === null) {
			this.replay = null;
			this.playing = false;
		}
		// Keys held through a seek play from its end.
		if (seeked && this.held) this.#ask({ press: this.held });
		this.#warm(seen);
	}

	/** The memory's map: the blocks the updates changed, warm, fading as more run. */
	#warm(seen: Seen) {
		const blocks = new Uint8Array(seen.blocks);
		const heat = new Float32Array(blocks.length);
		for (let i = 0; i < blocks.length; i++) {
			heat[i] = blocks[i] ? 1 : (this.heat[i] ?? 0) * (seen.ran.length ? 0.92 : 1);
		}
		this.heat = heat;
		// A Rust cart keeps its stack at the bottom (`cartridge_build::link`):
		// what it holds lies past it, when anything there moved.
		const block = this.console?.block ?? 1;
		const stack = this.regions.find(({ name }) => name === 'stack')?.size ?? STACK;
		const past = blocks.indexOf(1, Math.ceil(stack / block));
		const first = past >= 0 ? past : blocks.indexOf(1);
		if (this.#follow && seen.ran.length && first >= 0) {
			this.#follow = false;
			this.show(first * block);
		}
	}

	close() {
		cancelAnimationFrame(this.#frame);
		this.#worker.terminate();
	}
}

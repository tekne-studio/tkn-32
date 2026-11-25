// @ts-check
/**
 * The emulator's host on a JavaScript engine's own WebAssembly: V8 in Chrome
 * and node, JavaScriptCore in Safari and bun, SpiderMonkey in Firefox. It
 * gives the runner (`runner.wasm`, built beside this file) the engine it
 * asks for, and gives a page the runner's calls as plain JavaScript. Each
 * game is compiled and instantiated here, beside the runner, its tones
 * going straight to the runner's `hear`; the runner judges everything else.
 * The host only draws: it looks each pixel up in the colours the runner
 * resolved ({@link draw}).
 *
 * A page compiles a game at once only off its frame's thread: run the
 * emulator in a worker. Reading a cover or checking a file runs no game and
 * may run anywhere.
 *
 *   const emulator = await hosted(await WebAssembly.compileStreaming(fetch(runnerUrl)));
 *   emulator.open(1, file, seed, performance.now());
 *   for (const id of emulator.tick(performance.now())) paint(id, emulator.rgba(id));
 */

/**
 * The version of the imports this host gives the runner, module `engine`:
 * a runner that asks for another is refused.
 */
const ENGINE = 1;

/** How many bytes the colours of a screen shown are: 256 of four bytes. */
const COLOURS_BYTES = 256 * 4;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/**
 * What a cartridge's cover says of it, when its file has one.
 * @typedef {{ title: string, maker: string, year: number }} Said
 */

/**
 * A cartridge's cover, as a page shows it before anyone plays it.
 * @typedef {{ said: Said | null, rgba: Uint8Array<ArrayBuffer> }} Covered
 */

/**
 * The runner's exports, as the host calls them.
 * @typedef {{
 *   memory: WebAssembly.Memory,
 *   names(): number, contract(): number, ticks(): number, kept(): number, words(): number, keys(): number,
 *   shared(): number, pads(): number, pad_bits(): number,
 *   width(): number, height(): number, budget(): number, block(): number,
 *   engine(): number, most(): number, longest(): number, text(): number, buttons(): number, refusals(): number,
 *   tones(): number,
 *   pages(): number, table(): number, calls(): number, values(): number,
 *   room(len: number): number, file(len: number): number, plays(): number, name(len: number): number, out(): number,
 *   check(): number, prepare(): number,
 *   open(id: number, seed: number, watched: number, now: number): void,
 *   open_prepared(id: number, seed: number, watched: number, now: number): void,
 *   still(id: number, now: number): void, fail(id: number, now: number): void, close(id: number): void,
 *   press(id: number, buttons: number): void, pace(id: number, pace: number, now: number): void,
 *   script_room(count: number): number, script(id: number): void, scripted(id: number): number,
 *   step(id: number, count: number): number,
 *   tick(now: number): number, idle(): number, stopped(id: number): number, updates(id: number): number,
 *   shown(id: number): number, read(id: number): number, sound(id: number): number, stop(id: number): number,
 *   frames(id: number): number, ran(id: number, from: number): number, size(id: number): number,
 *   window(id: number, at: number, len: number): number, blocks(id: number): number, cover(): number,
 *   hear: Function
 * }} Exports
 */

/**
 * A game the engine holds: the functions the runner calls it by, its
 * memory and its meter.
 * @typedef {{ functions: (Function | undefined)[], memory: WebAssembly.Memory, meter: WebAssembly.Global }} Game
 */

/**
 * The screen of `shown` in colour, four bytes a pixel: each pixel's byte
 * looked up in the 256 colours before it, the whole of a host's drawing.
 * @param {Uint8Array<ArrayBuffer>} shown what {@link Emulator#shown} gives
 * @returns {Uint8Array<ArrayBuffer>}
 */
export function draw(shown) {
	const colours = new Uint32Array(shown.buffer, shown.byteOffset, COLOURS_BYTES / 4);
	const pixels = shown.subarray(COLOURS_BYTES);
	const rgba = new Uint32Array(pixels.length);
	for (let i = 0; i < pixels.length; i++) rgba[i] = colours[pixels[i]];
	return new Uint8Array(rgba.buffer);
}

/**
 * The emulator on this engine's WebAssembly, its runner instantiated.
 * @param {WebAssembly.Module} runner `runner.wasm`, compiled
 * @returns {Promise<Emulator>}
 */
export async function hosted(runner) {
	/** @type {Map<number, Game>} */
	const games = new Map();
	let next = 0;
	let whyNot = '';
	/** @type {Exports | undefined} */
	let runs;
	/** @type {{ functions: string[], meter: string, memory: string, module: string, tone: string } | undefined} */
	let names;
	const own = () => /** @type {Exports} */ (runs);
	const bytes = (/** @type {number} */ at, /** @type {number} */ len) => new Uint8Array(own().memory.buffer, at, len);
	const game = (/** @type {number} */ handle) => /** @type {Game} */ (games.get(handle));

	const engine = {
		/** @param {number} at @param {number} len */
		load(at, len) {
			const { functions, meter, memory, module, tone } = /** @type {NonNullable<typeof names>} */ (names);
			try {
				const compiled = new WebAssembly.Module(bytes(at, len));
				const instance = new WebAssembly.Instance(compiled, { [module]: { [tone]: own().hear } });
				const exported = instance.exports;
				const found = {
					functions: functions.map((name) => /** @type {Function | undefined} */ (exported[name])),
					memory: /** @type {WebAssembly.Memory} */ (exported[memory]),
					meter: /** @type {WebAssembly.Global} */ (exported[meter])
				};
				if (!(found.memory instanceof WebAssembly.Memory)) throw new Error(`no ${memory} after preparing`);
				if (!(found.meter instanceof WebAssembly.Global)) throw new Error(`no ${meter} after preparing`);
				games.set(next, found);
				return next++;
			} catch (error) {
				whyNot = error instanceof Error ? error.message : String(error);
				return -1;
			}
		},
		/** @param {number} at @param {number} max */
		why_not(at, max) {
			const said = encoder.encode(whyNot).subarray(0, max);
			bytes(at, said.length).set(said);
			return said.length;
		},
		/** @param {number} handle @param {number} index @param {number} arg @param {number} out */
		call(handle, index, arg, out) {
			const called = game(handle).functions[index];
			let got = 0;
			if (called) {
				try {
					got = called(arg) | 0;
				} catch {
					// A trap, or a stack the engine ran out of: either stops it.
					return 1;
				}
			}
			new DataView(own().memory.buffer).setUint32(out, got >>> 0, true);
			return 0;
		},
		/** @param {number} handle @param {number} from @param {number} at @param {number} len */
		read(handle, from, at, len) {
			const source = game(handle).memory.buffer;
			from >>>= 0;
			if (from + len > source.byteLength) return 0;
			bytes(at, len).set(new Uint8Array(source, from, len));
			return 1;
		},
		/** @param {number} handle */
		meter: (handle) => game(handle).meter.value,
		/** @param {number} handle @param {number} units */
		set_meter(handle, units) {
			game(handle).meter.value = units;
		},
		/** @param {number} handle */
		drop(handle) {
			games.delete(handle);
		}
	};

	const instance = await WebAssembly.instantiate(runner, { engine });
	runs = /** @type {Exports} */ (/** @type {unknown} */ (instance.exports));
	const asks = runs.engine();
	if (asks !== ENGINE) throw new Error(`the runner asks for engine ${asks}, and this host gives ${ENGINE}`);
	const emulator = new Emulator(runs);
	const lines = emulator.names();
	const [meter, memory, module, tone] = lines.slice(-4);
	names = { functions: lines.slice(0, -4), meter, memory, module, tone };
	return emulator;
}

/** The runner's calls, as a page makes them: carts by a name the page gives each, time on one clock in milliseconds. */
export class Emulator {
	/** @type {Exports} */
	#runner;
	/** The version of the contract it runs games of. */
	contract;
	/** The version of the imports the runner asks of this host, the one it gives. */
	engine;
	/** The updates a second a cart runs at its clock's pace of 1. */
	ticks;
	/** The most updates whose tones a cart keeps until {@link Emulator#sound} takes them. */
	kept;
	/** The words a tone is: `env.tone`'s arguments. */
	words;
	/**
	 * The keys one person plays the buttons by, by the names a page's
	 * keyboard events give them (`KeyboardEvent.code`), each to its button's
	 * bit: the first pad's.
	 * @type {ReadonlyMap<string, number>}
	 */
	keys;
	/**
	 * The keys two people sharing a keyboard play the buttons by, a half
	 * each, as {@link Emulator#keys} names them: the left half the first
	 * pad's, the right half the second's.
	 * @type {ReadonlyMap<string, number>}
	 */
	shared;
	/**
	 * The contract's buttons, each by its name to its bit in the first pad's
	 * byte.
	 * @type {ReadonlyMap<string, number>}
	 */
	buttons;
	/**
	 * The pads the buttons' number carries, a byte each from its lowest, and
	 * how far each pad's byte lies above the one before it, in bits.
	 */
	pads;
	padBits;
	/** A screen's pixels across and down. */
	width;
	height;
	/** The meter's units one update may spend. */
	budget;
	/** The bytes of memory a byte of {@link Emulator#blocks} stands for. */
	block;
	/** The most bytes a cartridge's file may have. */
	most;
	/** The most updates a script holds, the longest session the console keeps. */
	longest;
	/**
	 * Every code {@link Emulator#check} answers, once each: what a host has
	 * words for.
	 * @type {readonly string[]}
	 */
	refusals;
	/**
	 * What a host holds for a game: the calls to `env.tone` an update counts
	 * (a host that hears a game apart from the runner hands it no more a
	 * call), and what its engine holds, at the least and at the most, the
	 * pages of 64 KiB of its memory, the elements of its table, the calls of
	 * its deepest chain and the values that chain holds.
	 */
	limits;

	/** @param {Exports} runner */
	constructor(runner) {
		this.#runner = runner;
		this.contract = runner.contract();
		this.engine = runner.engine();
		this.ticks = runner.ticks();
		this.kept = runner.kept();
		this.words = runner.words();
		this.keys = this.#bits(runner.keys());
		this.shared = this.#bits(runner.shared());
		this.buttons = this.#bits(runner.buttons());
		this.pads = runner.pads();
		this.padBits = runner.pad_bits();
		this.width = runner.width();
		this.height = runner.height();
		this.budget = runner.budget();
		this.block = runner.block();
		this.most = runner.most();
		this.longest = runner.longest();
		this.refusals = /** @type {string} */ (this.#text(runner.refusals())).split('\n');
		this.limits = {
			tones: runner.tones(),
			pages: runner.pages(),
			table: runner.table(),
			calls: runner.calls(),
			values: runner.values()
		};
	}

	/**
	 * The lines the call that gave `len` gave, a name, a space and a bit each.
	 * @param {number} len
	 * @returns {Map<string, number>}
	 */
	#bits(len) {
		const lines = /** @type {string} */ (this.#text(len)).split('\n');
		return new Map(
			lines.map((line) => {
				const [name, bit] = line.split(' ');
				return /** @type {[string, number]} */ ([name, Number(bit)]);
			})
		);
	}

	/** @returns {string[]} the names a host finds a game's parts by, a line each */
	names() {
		return decoder.decode(this.#out(this.#runner.names()) ?? new Uint8Array()).split('\n');
	}

	/** @param {ArrayLike<number>} bytes */
	#room(bytes) {
		const at = this.#runner.room(bytes.length);
		new Uint8Array(this.#runner.memory.buffer, at, bytes.length).set(bytes);
	}

	/**
	 * A cartridge's file, for the next call that reads one: one past the
	 * most a file may have gets no room, and the runner refuses it by its size.
	 * @param {ArrayLike<number>} file
	 */
	#file(file) {
		const at = this.#runner.file(file.length);
		if (at) new Uint8Array(this.#runner.memory.buffer, at, file.length).set(file);
	}

	/** @param {string} name */
	#name(name) {
		const said = encoder.encode(name);
		const at = this.#runner.name(said.length);
		new Uint8Array(this.#runner.memory.buffer, at, said.length).set(said);
	}

	/**
	 * The bytes the call that gave `len` gave, copied out; null for -1.
	 * @param {number} len
	 * @returns {Uint8Array<ArrayBuffer> | null}
	 */
	#out(len) {
		if (len < 0) return null;
		return new Uint8Array(this.#runner.memory.buffer, this.#runner.out(), len).slice();
	}

	/** @param {number} len */
	#text(len) {
		const out = this.#out(len);
		return out && decoder.decode(out);
	}

	/** @param {number} len */
	#numbers(len) {
		const out = this.#out(len);
		return out && new Uint32Array(out.buffer);
	}

	/**
	 * The pads `file` plays, for a host to seat as many people before it
	 * runs it: 1 when it says none; null when {@link Emulator#check}
	 * refuses the file, which it judges as `check` does, in the same one
	 * pass.
	 * @param {ArrayLike<number>} file
	 */
	plays(file) {
		this.#file(file);
		const pads = this.#runner.plays();
		return pads < 0 ? null : pads;
	}

	/**
	 * Why `file` breaks the contract, as a host judges it before keeping it:
	 * the code of why not, or `''` when it keeps it.
	 * @param {ArrayLike<number>} file
	 */
	check(file) {
		this.#file(file);
		return this.#text(this.#runner.check()) ?? '';
	}

	/**
	 * `file` prepared, for a host that keeps prepared bytes by the file's
	 * hash; null when it breaks the contract.
	 * @param {ArrayLike<number>} file
	 */
	prepare(file) {
		this.#file(file);
		return this.#out(this.#runner.prepare());
	}

	/**
	 * Opens `file` as `id`, started with `seed`, its clock running from
	 * `now`: running, or stopped with its screen saying why. A cart
	 * `watched` is seen from inside.
	 * @param {number} id @param {ArrayLike<number>} file @param {number} seed @param {number} now
	 */
	open(id, file, seed, now, watched = false) {
		this.#file(file);
		this.#runner.open(id, seed, Number(watched), now);
	}

	/**
	 * {@link Emulator#open} for bytes {@link Emulator#prepare} gave. Bytes
	 * of another shape stop as a load that failed; bytes of its shape that
	 * preparing never gave may run unmetered, so a host keeps prepared
	 * bytes only where no one else can write them.
	 * @param {number} id @param {ArrayLike<number>} prepared @param {number} seed @param {number} now
	 */
	openPrepared(id, prepared, seed, now, watched = false) {
		this.#room(prepared);
		this.#runner.open_prepared(id, seed, Number(watched), now);
	}

	/**
	 * Shows the cover of `file`, whose hash is `name`, as `id`, still.
	 * @param {number} id @param {ArrayLike<number>} file @param {string} name @param {number} now
	 */
	still(id, file, name, now) {
		this.#file(file);
		this.#name(name);
		this.#runner.still(id, now);
	}

	/**
	 * Shows `id` as a cart whose file never came.
	 * @param {number} id @param {number} now
	 */
	fail(id, now) {
		this.#runner.fail(id, now);
	}

	/** @param {number} id */
	close(id) {
		this.#runner.close(id);
	}

	/**
	 * Holds the contract's `buttons`, every pad's, down on `id` from its next
	 * update on.
	 * @param {number} id @param {number} buttons
	 */
	press(id, buttons) {
		this.#runner.press(id, buttons);
	}

	/**
	 * Runs `id`'s clock at `pace` times the contract's from `now`; 0 holds it.
	 * @param {number} id @param {number} pace @param {number} now
	 */
	pace(id, pace, now) {
		this.#runner.pace(id, pace, now);
	}

	/**
	 * Plays `buttons` on `id`, one an update, after any script it plays: each
	 * the number `update` takes, every pad's buttons. A script past
	 * {@link Emulator#longest} gets no room, and throws.
	 * @param {number} id @param {ArrayLike<number>} buttons
	 */
	script(id, buttons) {
		// Past 4 G updates, any count past the longest is refused the same.
		const at = this.#runner.script_room(Math.min(buttons.length, 2 ** 32 - 1));
		if (!at) throw new Error(`a script of ${buttons.length} updates, past the ${this.longest} one holds`);
		// Read after the room is made, which may have grown the memory.
		const words = new DataView(this.#runner.memory.buffer, at, buttons.length * 4);
		for (let update = 0; update < buttons.length; update++) words.setUint32(update * 4, buttons[update] >>> 0, true);
		this.#runner.script(id);
	}

	/**
	 * How many updates of `id`'s script are left; null when it plays none.
	 * @param {number} id
	 */
	scripted(id) {
		const left = this.#runner.scripted(id);
		return left < 0 ? null : left;
	}

	/**
	 * Runs `id`'s next `count` updates now, outside its clock, at most
	 * {@link Emulator#kept} a call: how many ran.
	 * @param {number} id @param {number} count
	 */
	step(id, count) {
		return this.#runner.step(id, count);
	}

	/**
	 * Runs every cart's updates due by `now`: the carts whose screens changed.
	 * @param {number} now
	 * @returns {Uint32Array<ArrayBuffer>}
	 */
	tick(now) {
		return this.#numbers(this.#runner.tick(now)) ?? new Uint32Array();
	}

	/** Whether a tick would do nothing. */
	idle() {
		return this.#runner.idle() === 1;
	}

	/** @param {number} id */
	stopped(id) {
		return this.#runner.stopped(id) === 1;
	}

	/** @param {number} id @returns {number | null} */
	updates(id) {
		const updates = this.#runner.updates(id);
		return updates < 0 ? null : updates;
	}

	/**
	 * What `id`'s screen is drawn from: 256 colours, then the pixels.
	 * @param {number} id
	 */
	shown(id) {
		return this.#out(this.#runner.shown(id));
	}

	/**
	 * `id`'s screen in colour, four bytes a pixel.
	 * @param {number} id
	 */
	rgba(id) {
		const shown = this.shown(id);
		return shown && draw(shown);
	}

	/**
	 * `id`'s screen as the game wrote it, what an agent reads: its palette,
	 * then its pixels.
	 * @param {number} id
	 */
	read(id) {
		return this.#out(this.#runner.read(id));
	}

	/**
	 * What `id` played since last asked, as the chip's processor takes it;
	 * empty when it ran no update.
	 * @param {number} id
	 * @returns {Uint32Array<ArrayBuffer>}
	 */
	sound(id) {
		return this.#numbers(this.#runner.sound(id)) ?? new Uint32Array();
	}

	/**
	 * Each update's tones in a batch {@link Emulator#sound} gave: a list an
	 * update, in order, {@link Emulator#words} words a tone.
	 * @param {Uint32Array} batch
	 * @returns {Uint32Array<ArrayBuffer>[]}
	 */
	tones(batch) {
		const updates = [];
		for (let at = 1; at < batch.length; ) {
			const count = batch[at++];
			updates.push(batch.slice(at, at + count * this.words));
			at += count * this.words;
		}
		return updates;
	}

	/**
	 * Why `id` stopped, as its screen says it; null while it runs.
	 * @param {number} id
	 */
	stop(id) {
		return this.#text(this.#runner.stop(id)) || null;
	}

	/**
	 * The frames hash of a watched `id`.
	 * @param {number} id
	 */
	frames(id) {
		return this.#text(this.#runner.frames(id));
	}

	/**
	 * Every update a watched `id` ran from update `from`: its buttons and
	 * the units it spent, a pair of words each.
	 * @param {number} id @param {number} from
	 */
	ran(id, from) {
		return this.#numbers(this.#runner.ran(id, from));
	}

	/**
	 * The bytes of memory a watched `id` holds.
	 * @param {number} id
	 */
	size(id) {
		const size = this.#runner.size(id);
		return size < 0 ? null : size;
	}

	/**
	 * `len` bytes of a watched `id`'s memory from `at`, and a byte each, 1
	 * where the last updates changed it.
	 * @param {number} id @param {number} at @param {number} len
	 */
	window(id, at, len) {
		const out = this.#out(this.#runner.window(id, at, len));
		if (!out) return null;
		const half = out.length / 2;
		return { bytes: out.slice(0, half), changes: out.slice(half) };
	}

	/**
	 * A watched `id`'s memory's map: a byte a block, 1 where the last
	 * updates changed it.
	 * @param {number} id
	 */
	blocks(id) {
		return this.#out(this.#runner.blocks(id));
	}

	/**
	 * What a page shows of `file`, whose hash is `name`, before it runs:
	 * read from the file, never by running it.
	 * @param {ArrayLike<number>} file @param {string} name
	 * @returns {Covered}
	 */
	cover(file, name) {
		this.#file(file);
		this.#name(name);
		const out = /** @type {Uint8Array<ArrayBuffer>} */ (this.#out(this.#runner.cover()));
		const shownBytes = COLOURS_BYTES + this.width * this.height;
		const rgba = draw(out.subarray(0, shownBytes));
		if (out.length === shownBytes) return { said: null, rgba };
		const view = new DataView(out.buffer, shownBytes);
		const year = view.getUint16(0, true);
		let at = shownBytes + 2;
		const text = () => {
			const len = out[at];
			const said = decoder.decode(out.subarray(at + 1, at + 1 + len));
			at += 1 + len;
			return said;
		};
		const title = text();
		return { said: { title, maker: text(), year }, rgba };
	}

	/**
	 * `line` in the console's font: its size, and a byte a pixel, a row
	 * after another, 1 where the font inks.
	 * @param {string} line
	 * @returns {{ width: number, height: number, ink: Uint8Array<ArrayBuffer> }}
	 */
	text(line) {
		this.#room(encoder.encode(line));
		const out = /** @type {Uint8Array<ArrayBuffer>} */ (this.#out(this.#runner.text()));
		const view = new DataView(out.buffer);
		return { width: view.getUint16(0, true), height: view.getUint16(2, true), ink: out.slice(4) };
	}
}

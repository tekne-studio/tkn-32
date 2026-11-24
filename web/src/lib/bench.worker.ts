/**
 * The worker that holds the cartridge on the bench: the emulator `bun run
 * workbench` builds into `./pkg` (`scripts/emulator.ts`), its runner run by
 * its own host on the browser's WebAssembly, as a cabinet runs one. The
 * page opens a file, watched, then tells it each frame it drew: the game's
 * clock, the shelf's, runs what is due at its pace, and the worker answers
 * with everything the page shows whenever updates ran. Off the page's
 * thread: the browser compiles a cartridge at once only there, and an
 * update never shares the page's frame.
 */
import type { Emulator } from '../../../crates/runner/host.js';
import type { Asked, Seen, Told } from './bench';

type Host = typeof import('../../../crates/runner/host.js');

/** The cart the bench holds, by the name the emulator knows it by. */
const ID = 1;
/** The most updates one answer runs while the game seeks, so the page shows it going. */
const SEEK = 600;
/**
 * When the clock last ran what was due: a game stopped is held there, as
 * last shown, so no update falls due after it.
 */
let ticked = 0;

interface Scope {
	postMessage(message: Told, transfer?: Transferable[]): void;
	onmessage: ((event: MessageEvent<Asked>) => void) | null;
}
const scope = self as unknown as Scope;

// Globs resolve to nothing when the emulator was never built: the page then
// says so.
const manifests = import.meta.glob<{ runner: string; host: string }>('./pkg/emulator.json', {
	eager: true,
	import: 'default'
});
const files = import.meta.glob<string>('./pkg/*.{wasm,js}', { eager: true, query: '?url', import: 'default' });

async function load(): Promise<Emulator> {
	const manifest = manifests['./pkg/emulator.json'];
	if (!manifest) throw new Error('the emulator was never built: bun run workbench');
	const url = (name: string) => files[`./pkg/${name}`];
	const [{ hosted }, runner] = await Promise.all([
		import(/* @vite-ignore */ url(manifest.host)) as Promise<Host>,
		fetch(url(manifest.runner))
			.then((answer) => answer.arrayBuffer())
			.then((bytes) => WebAssembly.compile(bytes))
	]);
	return hosted(runner);
}

/** The game the bench holds, as the page stamped its open. */
let game = 0;
let shown = { at: 0, len: 512 };
/** The update a seek runs to. */
let seeking: number | null = null;
/** The updates already said to the page. */
let said = 0;

/**
 * Everything the page shows, the updates run since the last answer among
 * it, their sound left out when `silent`.
 */
function seen(emulator: Emulator, ticked: boolean, silent: boolean): [Seen, Transferable[]] {
	const read = emulator.read(ID) ?? new Uint8Array();
	const pixels = emulator.width * emulator.height;
	const indices = read.slice(read.length - pixels).buffer;
	const rgba = (emulator.rgba(ID) ?? new Uint8Array()).buffer;
	const pairs = emulator.ran(ID, said) ?? new Uint32Array();
	const ran = Array.from({ length: pairs.length / 2 }, (_, at) => ({ buttons: pairs[2 * at], spent: pairs[2 * at + 1] }));
	said += ran.length;
	const played = emulator.sound(ID);
	const sound = !silent && played.length ? played.buffer : null;
	const window = emulator.window(ID, shown.at, shown.len) ?? { bytes: new Uint8Array(), changes: new Uint8Array() };
	const blocks = (emulator.blocks(ID) ?? new Uint8Array()).buffer;
	const answer: Seen = {
		game,
		ticked,
		rgba,
		indices,
		palette: Array.from(read.subarray(0, read.length - pixels)),
		updates: emulator.updates(ID) ?? 0,
		hash: emulator.frames(ID) ?? '',
		stop: emulator.stop(ID),
		ran,
		tones: emulator.tones(played).map((words) => Array.from(words)),
		sound,
		scripted: emulator.scripted(ID),
		seeking,
		size: emulator.size(ID) ?? 0,
		window: { at: shown.at, bytes: window.bytes.buffer, changes: window.changes.buffer },
		blocks
	};
	const transfer = [rgba, indices, window.bytes.buffer, window.changes.buffer, blocks, ...(sound ? [sound] : [])];
	return [answer, transfer];
}

function post(emulator: Emulator, ticked = false, silent = false) {
	const [answer, transfer] = seen(emulator, ticked, silent);
	scope.postMessage(answer, transfer);
}

/**
 * Opens `file` watched, as a cabinet shows it first: its cover read too. A
 * file is named by the SHA-256 of its bytes.
 */
async function open(emulator: Emulator, data: Extract<Asked, { open: ArrayBuffer }>) {
	const file = new Uint8Array(data.open);
	const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', file));
	const name = Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('');
	const now = performance.now();
	game = data.game;
	said = 0;
	seeking = data.seek && data.seek > 0 ? data.seek : null;
	ticked = now;
	emulator.open(ID, file, data.seed, now, true);
	if (data.script) emulator.script(ID, data.script);
	emulator.pace(ID, seeking === null ? data.pace : 0, now);
	const cover = emulator.cover(file, name);
	const [answer, transfer] = seen(emulator, false, true);
	scope.postMessage({ ...answer, cover }, [...transfer, cover.rgba.buffer]);
}

/**
 * Runs what the clock has due, or the next stretch of a seek, silent, and
 * answers either way.
 */
function tick(emulator: Emulator) {
	if (seeking !== null) {
		const updates = emulator.updates(ID) ?? 0;
		const ran = emulator.step(ID, Math.min(SEEK, seeking - updates));
		if (ran === 0 || updates + ran >= seeking) seeking = null;
		post(emulator, true, true);
	} else if (emulator.tick((ticked = performance.now())).length) {
		post(emulator, true);
	} else {
		scope.postMessage({ ticked: true });
	}
}

function answer(emulator: Emulator, data: Asked) {
	if ('open' in data) return open(emulator, data);
	if ('tick' in data) tick(emulator);
	else if ('pace' in data) emulator.pace(ID, data.pace, data.pace === 0 ? ticked : performance.now());
	else if ('press' in data) emulator.press(ID, data.press);
	else if ('step' in data) {
		if (emulator.step(ID, data.step)) post(emulator);
	} else if ('toEnd' in data) {
		const left = emulator.scripted(ID);
		if (left !== null) seeking = (emulator.updates(ID) ?? 0) + left;
	} else {
		shown = data.window;
		post(emulator);
	}
}

function failed(error: unknown) {
	scope.postMessage({ failed: error instanceof Error ? error.message : String(error) });
}

/**
 * One message at a time, in the order asked: an open waits on its digest,
 * and nothing asked after it may be answered before it.
 */
const loaded = load();
let queue: Promise<unknown> = loaded.then(({ width, height, ticks, budget, block, keys }) => {
	scope.postMessage({ ready: { width, height, ticks, budget, block, keys: [...keys] } });
}, failed);
scope.onmessage = ({ data }) => {
	queue = queue.then(() => loaded.then((emulator) => answer(emulator, data))).catch(failed);
};

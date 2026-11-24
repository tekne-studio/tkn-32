/**
 * The worker that holds the cartridge on the bench: the `Workbench` of
 * `crates/workbench`, built by `bun run workbench` into `./pkg`, on the
 * browser's own WebAssembly, as a cabinet runs one. The page opens a file,
 * then asks for updates, each with the buttons held, and for the stretch of
 * memory it shows; after each run it answers with everything it shows.
 * Off the page's thread: the browser compiles a cartridge at once only
 * there, and an update never shares the page's frame.
 */
import init, { Workbench, block, budget, cover } from './pkg/workbench.js';
import type { Asked, Covered, Seen } from './bench';

interface Scope {
	postMessage(message: Seen, transfer?: Transferable[]): void;
	onmessage: ((event: MessageEvent<Asked>) => void) | null;
}
const scope = self as unknown as Scope;

const ready = init();
let bench: Workbench | undefined;
/** The game the bench holds, as the page stamped its open. */
let game = 0;
let shown = { at: 0, len: 512 };

/** Everything the page shows, after `spent` lists each update just run. */
function seen(spent: number[], tones: number[][]): Seen {
	const b = bench!;
	return {
		game,
		rgba: b.rgba().buffer as ArrayBuffer,
		indices: b.indices().buffer as ArrayBuffer,
		palette: Array.from(b.palette()),
		updates: b.updates(),
		hash: b.hash(),
		stop: b.stop() ?? null,
		spent,
		tones,
		size: b.size(),
		window: {
			at: shown.at,
			bytes: b.memory(shown.at, shown.len).buffer as ArrayBuffer,
			changes: b.changes(shown.at, shown.len).buffer as ArrayBuffer
		},
		blocks: b.blocks().buffer as ArrayBuffer,
		block: block(),
		budget: budget()
	};
}

/**
 * The cover of `file`, as a cabinet shows it: `cartridge-web`'s reading,
 * which the bench links. A file is named by the SHA-256 of its bytes.
 */
async function covered(file: Uint8Array<ArrayBuffer>): Promise<Covered> {
	const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', file));
	const name = Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('');
	const read = cover(file, name);
	const title = read.title();
	const covered = title === undefined
		? null
		: { title, maker: read.maker() ?? '', year: read.year() ?? 0 };
	const rgba = read.rgba().buffer as ArrayBuffer;
	read.free();
	return { said: covered, rgba };
}

function post(message: Seen) {
	scope.postMessage(message, [
		...(message.cover ? [message.cover.rgba] : []),
		message.rgba,
		message.indices,
		message.window.bytes,
		message.window.changes,
		message.blocks
	]);
}

/**
 * One message at a time, in the order asked: an open waits on its digest,
 * and nothing asked after it may be answered before it.
 */
let queue = ready.then(() => {});
scope.onmessage = ({ data }) => {
	queue = queue.then(() => answer(data));
};

async function answer(data: Asked) {
	if ('open' in data) {
		bench?.free();
		const file = new Uint8Array(data.open);
		bench = new Workbench(file, data.seed);
		game = data.game;
		post({ ...seen([], []), cover: await covered(file) });
	} else if ('run' in data) {
		if (!bench) return;
		const spent: number[] = [];
		const tones: number[][] = [];
		for (const buttons of data.run) {
			bench.update(buttons);
			spent.push(bench.spent());
			tones.push(Array.from(bench.tones()));
		}
		post(seen(spent, tones));
	} else if ('window' in data) {
		shown = data.window;
		if (bench) post(seen([], []));
	}
}

// The workbench: a page that runs a cartridge and shows it from inside.
// Builds the bench's engine (`crates/workbench`, through wasm-bindgen) with
// the emulator, whose chip it sounds through (`scripts/emulator.ts`), and
// every cart under examples/, puts the carts where the page's server serves
// them, and starts the page on the first free port from 5180, opened on the
// cart named:
//   bun run workbench            (rocks)
//   bun run workbench dot
//   bun run workbench ../mygame   (a cart in another workspace, by its folder)
// Then it watches every cart's folder: a file saved there rebuilds that
// cart, and the page runs the new build through the game played so far. A
// cart named by its folder is built in its own workspace and served in place
// of the one of its name here, so a cart at work elsewhere is heard as it
// changes.
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, watch } from 'node:fs';
import { $ } from 'bun';
import { BUILT, build, built, cartNamed, examples, type Cart } from './carts';
import { buildEmulator } from './emulator';
import { freePort, HOST, ROOT } from './lib';
import { regions } from './linkmap';

export const WEB = `${ROOT}/web`;
/** Where the built carts wait for the page's server (`web/vite.config.ts`). */
const SERVED = `${ROOT}/target/workbench`;

/**
 * Puts a built cart where the page's server serves it, with the regions of
 * its memory its last link mapped (`cartridge_build::link`), none for a cart
 * that does not call it.
 */
async function serve(cart: Cart) {
	await Bun.write(`${SERVED}/${cart.name}.wasm`, Bun.file(built(cart)));
	const maps = [...new Bun.Glob(`${BUILT}/build/${cart.name}-*/out/link.map`).scanSync(cart.root)]
		.map((map) => `${cart.root}/${map}`)
		.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
	const named = maps[0] ? regions(readFileSync(maps[0], 'utf8')) : [];
	await Bun.write(`${SERVED}/${cart.name}.symbols.json`, JSON.stringify(named));
}

/**
 * Builds the bench's engine (through wasm-bindgen) beside the emulator, and
 * puts into the page `web` each file of them that changed: all of them, for
 * the page to run, or the `types` alone, what a check reads.
 */
export async function buildEngine(web = WEB, what: 'all' | 'types' = 'all') {
	const emulator = `${ROOT}/target/emulator`;
	const made = `${ROOT}/target/workbench-pkg`;
	await buildEmulator(emulator);
	await $`cargo build --release --target wasm32-unknown-unknown -p workbench`.cwd(ROOT);
	await $`wasm-bindgen --target web --out-dir ${made} ${ROOT}/${BUILT}/workbench.wasm`.cwd(ROOT);
	const files = readdirSync(made).map((file) => [`${made}/${file}`, file]);
	// The chip as every page's AudioWorklet runs it, the emulator's own
	// files: plain exports, no glue, and the processor that runs them.
	files.push(...['chip_worklet.wasm', 'chip-worklet.js'].map((file) => [`${emulator}/${file}`, file]));
	mkdirSync(`${web}/src/lib/pkg`, { recursive: true });
	for (const [from, file] of files) {
		if (what === 'types' && !file.endsWith('.d.ts')) continue;
		const to = `${web}/src/lib/pkg/${file}`;
		const bytes = readFileSync(from);
		if (!existsSync(to) || !bytes.equals(readFileSync(to))) await Bun.write(to, bytes);
	}
}

/** Builds every cart and puts it where the page serves it, the carts named by their folder in place of those of their names here. */
async function buildCarts(outside: Cart[]): Promise<Cart[]> {
	const carts = [...(await examples()).filter(({ name }) => !outside.some((cart) => cart.name === name)), ...outside];
	await build(carts);
	rmSync(SERVED, { recursive: true, force: true });
	mkdirSync(SERVED, { recursive: true });
	for (const cart of carts) await serve(cart);
	await Bun.write(`${SERVED}/carts.json`, JSON.stringify(carts.map(({ name, called }) => ({ name, called }))));
	return carts;
}

if (import.meta.main) {
	const asked = process.argv[2] ?? 'rocks';
	const opened = await cartNamed(asked);
	const outside = asked.includes('/') ? [opened] : [];
	const cart = opened.name;
	await buildEngine();
	const carts = await buildCarts(outside);
	const port = freePort(5180);
	console.log(`workbench: http://${HOST}:${port}/?cart=${cart}`);
	const dev = Bun.spawn(['bun', 'run', 'dev', '--host', HOST, '--port', String(port), '--strictPort'], {
		cwd: WEB,
		env: { ...process.env, WORKBENCH_CARTS: SERVED },
		stdout: 'inherit',
		stderr: 'inherit'
	});
	process.on('SIGINT', () => dev.kill());
	watchCarts(carts);
	process.exit(await dev.exited);
}

/**
 * Rebuilds a cart when a file in its folder changes, one build at a time,
 * and says how it went in `build.json` beside the carts, which the page's
 * server passes on to the page (`web/vite.config.ts`).
 */
function watchCarts(carts: Cart[]) {
	const waiting = new Map<string, Timer>();
	let building = Promise.resolve();
	const rebuild = (cart: Cart) => {
		const { name } = cart;
		building = building.then(async () => {
			const run = await $`cargo build --release --target wasm32-unknown-unknown -p ${name}`.cwd(cart.root).quiet().nothrow();
			const said = run.stderr.toString();
			const ok = run.exitCode === 0;
			if (ok) await serve(cart);
			// The compiler's words from its first error on, or its last lines.
			const from = said.search(/^error/m);
			const message = ok ? '' : (from >= 0 ? said.slice(from) : said).split('\n').slice(0, 60).join('\n');
			await Bun.write(`${SERVED}/build.json`, JSON.stringify({ name, ok, message, at: Date.now() }));
			console.log(ok ? `workbench: ${name} rebuilt` : `workbench: ${name} failed to build`);
		});
	};
	for (const cart of carts) {
		watch(cart.dir, { recursive: true }, (_, file) => {
			if (!file || file.startsWith('target')) return;
			clearTimeout(waiting.get(cart.name));
			waiting.set(cart.name, setTimeout(() => rebuild(cart), 150));
		});
	}
}

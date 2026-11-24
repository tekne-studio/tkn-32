// The workbench: a page that runs a cartridge and shows it from inside, on
// the emulator a cabinet runs. Builds the emulator (`scripts/emulator.ts`)
// into the page and every cart under examples/, puts the carts where the
// page's server serves them, and starts the page on the first free port
// from 5180, opened on the cart named:
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
import { buildEmulator, EMULATOR } from './emulator';
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
 * Builds the emulator and puts it into the page `web` as it is, its files
 * named by their hash beside `emulator.json`: each file that changed, and
 * none of an older build left.
 */
export async function buildEmulatorInto(web = WEB) {
	const manifest = await buildEmulator(EMULATOR);
	const pkg = `${web}/src/lib/pkg`;
	const files = ['emulator.json', manifest.runner, manifest.host, manifest.chip, manifest.processor];
	mkdirSync(pkg, { recursive: true });
	for (const file of readdirSync(pkg)) if (!files.includes(file)) rmSync(`${pkg}/${file}`);
	for (const file of files) {
		const to = `${pkg}/${file}`;
		const bytes = readFileSync(`${EMULATOR}/${file}`);
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
	await buildEmulatorInto();
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

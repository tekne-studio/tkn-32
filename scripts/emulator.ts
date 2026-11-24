// The emulator a page loads to run cartridges, built whole into one folder:
// the runner on the browser's own WebAssembly, `crates/cartridge-web` bound
// by wasm-bindgen (`cartridge_web.js`, `cartridge_web_bg.wasm`), and the chip
// a game sounds through, its module and the processor an AudioWorklet runs
// it by (`chip_worklet.wasm`, `chip-worklet.js`). The workbench sounds
// through this folder's chip; a host copies the folder as it is.
//   bun scripts/emulator.ts <folder>
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { $ } from 'bun';
import { ROOT } from './lib';

const BUILT = `${ROOT}/target/wasm32-unknown-unknown/release`;

/** The packages the emulator is built from. */
const PACKAGES = ['cartridge-web', 'chip-worklet'];

/**
 * Builds the emulator into `out`, in a cargo build of its own: built beside
 * another package, the features they share would be unified, and the
 * emulator would be what that build made it, not the one every host runs.
 */
export async function buildEmulator(out: string) {
	if (!Bun.which('wasm-bindgen')) throw new Error('wasm-bindgen not found: run `bun run setup`');
	const packages = PACKAGES.flatMap((name) => ['-p', name]);
	await $`cargo build --release --target wasm32-unknown-unknown ${packages}`.cwd(ROOT);
	mkdirSync(out, { recursive: true });
	await $`wasm-bindgen --target web --out-dir ${out} ${BUILT}/cartridge_web.wasm`;
	await Bun.write(`${out}/chip_worklet.wasm`, Bun.file(`${BUILT}/chip_worklet.wasm`));
	await Bun.write(`${out}/chip-worklet.js`, Bun.file(`${ROOT}/crates/chip-worklet/worklet.js`));
}

if (import.meta.main) {
	const [out] = process.argv.slice(2);
	if (!out) throw new Error('bun scripts/emulator.ts <folder>');
	await buildEmulator(resolve(out));
	console.log(`the emulator -> ${resolve(out)}`);
}

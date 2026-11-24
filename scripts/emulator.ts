// The emulator, built whole into one folder, each file named by the SHA-256
// of its bytes: the runner every host runs (`crates/runner`), its host on a
// JavaScript engine's own WebAssembly (`crates/runner/host.js`), and the
// chip a game sounds through with the processor an AudioWorklet runs it by
// (`crates/chip-worklet`). `emulator.json` names each file and the version
// of the contract it runs. A host takes the folder as it is and pins it by
// its runner's hash; the workbench and the tests run the one it builds into
// target/emulator.
//
// The build is the same bytes on every machine on the pinned compiler
// (rust-toolchain.toml): it runs in a target folder of its own, with every
// path the machine gives the compiler, this folder, cargo's sources and the
// standard library's, written as the same name everywhere, so a panic's
// location names no one's home. A pin is proved its source's by building
// again and comparing (`--same`).
//   bun run emulator [folder]
//   bun run emulator --same <folder>    fails naming each file that differs
import { mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { $ } from 'bun';
import { ROOT } from './lib';

/** The emulator's own target folder: its flags never rebuild another build's crates. */
const TARGET = `${ROOT}/target/emulator-build`;
const BUILT = `${TARGET}/wasm32-unknown-unknown/release`;
/** Where the console's own build of the emulator goes: the workbench's and the tests'. */
export const EMULATOR = `${ROOT}/target/emulator`;

/** The packages the emulator is built from. */
const PACKAGES = ['runner', 'chip-worklet'];

/**
 * What `emulator.json` says: the versions of the contract and of the
 * imports the runner asks of a host's engine, each file by its name in the
 * folder, and the numbers a host may need before it runs the runner, each
 * by the name of the runner's export that gives it: a screen's size, the
 * updates a second, the most bytes a file may have, the calls of `env.tone`
 * an update counts, what a host's engine holds for a game (`pages` of its
 * memory, its `table`, the `calls` and `values` of its deepest chain), the
 * contract's buttons by their names, and the keys a keyboard plays them by,
 * each to its button's bit.
 */
export interface Manifest {
	contract: number;
	engine: number;
	runner: string;
	host: string;
	chip: string;
	processor: string;
	cases: string;
	width: number;
	height: number;
	ticks: number;
	most: number;
	tones: number;
	pages: number;
	table: number;
	calls: number;
	values: number;
	buttons: Record<string, number>;
	keys: Record<string, number>;
}

/** The SHA-256 of `bytes`, in hex: what a file of the emulator is named by. */
export function sha256(bytes: Uint8Array): string {
	return new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
}

/** Writes `bytes` into `out` under their hash and `ext`: the name written. */
async function keep(out: string, bytes: Uint8Array, ext: string): Promise<string> {
	const name = `${sha256(bytes)}.${ext}`;
	await Bun.write(`${out}/${name}`, bytes);
	return name;
}

/**
 * The compiler's flags that write each machine's paths as one name: this
 * folder as `/tkn-32`, cargo's home as `/cargo`, and the standard library's
 * sources, which a machine with them installed would name in its own
 * place, as the compiler's release names them. Encoded as cargo takes a
 * list, so a path with a space stays one.
 */
async function remapped(): Promise<string> {
	const sysroot = (await $`rustc --print sysroot`.cwd(ROOT).text()).trim();
	const commit = (await $`rustc -vV`.cwd(ROOT).text()).match(/^commit-hash: (\w+)$/m)?.[1];
	if (!commit) throw new Error('rustc -vV names no commit');
	const cargo = process.env.CARGO_HOME ?? `${homedir()}/.cargo`;
	const prefixes: [string, string][] = [
		[`${sysroot}/lib/rustlib/src/rust`, `/rustc/${commit}`],
		[cargo, '/cargo'],
		[ROOT, '/tkn-32']
	];
	return prefixes.map(([from, to]) => `--remap-path-prefix=${from}=${to}`).join('\x1f');
}

/** A file of an emulator: its hash, then what it is. */
const BUILT_FILE = /^[0-9a-f]{64}\.(wasm|js|json)$/;

/**
 * Builds the emulator into `out`, in a cargo build of its own: built beside
 * another package, the features they share would be unified, and the
 * emulator would be what that build made it, not the one every host runs.
 * Writes its files and `emulator.json`, and removes the files of an older
 * build the manifest no longer names, so the folder is one emulator.
 */
export async function buildEmulator(out: string): Promise<Manifest> {
	const packages = PACKAGES.flatMap((name) => ['-p', name]);
	const env = { ...process.env, CARGO_TARGET_DIR: TARGET, CARGO_ENCODED_RUSTFLAGS: await remapped() };
	await $`cargo build --release --target wasm32-unknown-unknown ${packages}`.cwd(ROOT).env(env);
	mkdirSync(out, { recursive: true });
	const read = async (file: string) => new Uint8Array(await Bun.file(file).arrayBuffer());
	const runner = await read(`${BUILT}/runner.wasm`);
	const { hosted } = await import(`${ROOT}/crates/runner/host.js`);
	const emulator = await hosted(new WebAssembly.Module(runner));
	const manifest: Omit<Manifest, 'cases'> = {
		contract: emulator.contract,
		engine: emulator.engine,
		runner: await keep(out, runner, 'wasm'),
		host: await keep(out, await read(`${ROOT}/crates/runner/host.js`), 'js'),
		chip: await keep(out, await read(`${BUILT}/chip_worklet.wasm`), 'wasm'),
		processor: await keep(out, await read(`${ROOT}/crates/chip-worklet/worklet.js`), 'js'),
		width: emulator.width,
		height: emulator.height,
		ticks: emulator.ticks,
		most: emulator.most,
		...emulator.limits,
		buttons: Object.fromEntries(emulator.buttons),
		keys: Object.fromEntries(emulator.keys)
	};
	// The suites run this build to gather its cases, so it is whole first.
	await Bun.write(`${out}/emulator.json`, `${JSON.stringify(manifest, null, '\t')}\n`);
	const whole: Manifest = { ...manifest, cases: await keep(out, await gatherCases(out), 'json') };
	await Bun.write(`${out}/emulator.json`, `${JSON.stringify(whole, null, '\t')}\n`);
	const named = new Set(Object.values(whole).filter((value) => typeof value === 'string'));
	for (const file of readdirSync(out)) {
		if (BUILT_FILE.test(file) && !named.has(file)) rmSync(`${out}/${file}`);
	}
	return whole;
}

/** The suites whose every run is a case: what a host is held to. */
const SUITES = ['hostile', 'contract'];

/**
 * The emulator's cases, what any host is held to: every run of the hostile
 * and contract suites, on the emulator in `out`, which wasmi alone and
 * every host at hand agreed on, each with the test it came from, the cart's
 * file in hex, its seed, its buttons as runs of `[value, count]`, a byte an
 * update, and the run a host must give: the FNV-1a of every screen shown
 * after an update (its pixels, then its palette), of every tone kept (its
 * words, little end first), why it stopped (`running` while it runs), the
 * updates it ran, and the FNV-1a of its last screen in colour. A case a
 * host cannot run is a host that fails. Sorted, so the same suites give
 * the same bytes.
 */
async function gatherCases(out: string): Promise<Uint8Array> {
	const folder = `${TARGET}/cases`;
	rmSync(folder, { recursive: true, force: true });
	const tests = SUITES.flatMap((suite) => ['--test', suite]);
	const { CARGO_TARGET_DIR: _, CARGO_ENCODED_RUSTFLAGS: __, ...env } = process.env;
	await $`cargo test -q -p cartridge-wasmi ${tests}`.cwd(ROOT).env({ ...env, CASES: folder, EMULATOR: out }).quiet();
	const cases = readdirSync(folder)
		.map((file) => readFileSync(`${folder}/${file}`, 'utf8'))
		.sort();
	if (cases.length === 0) throw new Error(`the ${SUITES.join(' and ')} suites kept no case`);
	return new TextEncoder().encode(`[\n${cases.join(',\n')}\n]\n`);
}

/**
 * Builds the emulator into a folder of its own and compares it with the
 * one at `pinned`: the names of the files that differ, empty when the pin
 * is what this source builds.
 */
export async function differences(pinned: string): Promise<string[]> {
	const built = await buildEmulator(`${TARGET}/same`);
	const read = async (folder: string) =>
		JSON.parse(await Bun.file(`${folder}/emulator.json`).text()) as Manifest;
	const kept = await read(pinned);
	return (Object.keys(built) as (keyof Manifest)[])
		.filter((part) => JSON.stringify(built[part]) !== JSON.stringify(kept[part]))
		.map((part) => `${part}: pinned ${JSON.stringify(kept[part])}, built ${JSON.stringify(built[part])}`);
}

if (import.meta.main) {
	const args = process.argv.slice(2);
	if (args[0] === '--same') {
		const pinned = resolve(args[1] ?? '');
		const differ = await differences(pinned);
		if (differ.length > 0) {
			console.error(`the emulator at ${pinned} is not what this source builds:\n  ${differ.join('\n  ')}`);
			process.exit(1);
		}
		console.log(`the emulator at ${pinned} is what this source builds`);
	} else {
		const out = args[0] ? resolve(args[0]) : EMULATOR;
		await buildEmulator(out);
		console.log(`the emulator -> ${out}`);
	}
}

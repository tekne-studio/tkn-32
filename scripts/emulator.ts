// The emulator, built whole into one folder, each file named by the SHA-256
// of its bytes: the runner every host runs (`crates/runner`), its host on a
// JavaScript engine's own WebAssembly (`crates/runner/host.js`), and the
// chip a game sounds through with the processor an AudioWorklet runs it by
// (`crates/chip-worklet`). `emulator.json` names each file and the version
// of the contract it runs. A host takes the folder as it is and pins it by
// its runner's hash; the workbench and the tests run the one it builds into
// target/emulator.
//
// The build runs on the pinned compiler (rust-toolchain.toml) in a target
// folder of its own, with every path the machine gives the compiler, this
// folder, cargo's sources and the standard library's, written as the same
// name everywhere, so the folder it is built from changes nothing and a
// panic's location names no one's home. Its bytes are still the platform's
// that builds it: cargo hashes the build's machine into each crate under a
// build script, which moves the runner's symbols and the order of its
// functions. So a pin's runner and chip are built on one platform, `IMAGE`,
// Linux on x86-64, by Docker on any machine, natively on that one and
// emulated on another; its host and processor are the source's files as
// they are, and its cases come from the suites on the machine at hand,
// running the image's runner. `--same` builds and gathers it all again and
// compares every file, the cases too. A step in the image past `HANG` is
// taken as hung and fails.
//   bun run emulator [folder]           this machine's build, for work: the workbench, the tests
//   bun run emulator --pin <folder>     the build in the pin's image: what a host pins
//   bun run emulator --same <folder>    builds in the pin's image, fails naming each file that differs
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
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
 * The image a pin is built in, by the digest of its Linux x86-64 build: the
 * compiler rust-toolchain.toml names, from the official image's mirror on
 * ECR Public, which answers where Docker Hub may not.
 */
const IMAGE =
	'public.ecr.aws/docker/library/rust:1.100.0@sha256:1e626a07e55b67d7fe19de7a590519e0cf4b75cf3788b058fa75cf4d599d9b5a';
const PLATFORM = 'linux/amd64';
/** Where the console is in the image's container, and its build. */
const INSIDE = '/tkn-32';
const INSIDE_TARGET = '/target';
/**
 * The longest a step in the image may take, in seconds, emulated included:
 * past it the step is taken as hung and fails, and so does the build that
 * waits on its lock.
 */
const HANG = 15 * 60;

/** `command` in the image's shell, failing with `what` when it runs past `HANG`. */
function bounded(command: string, what: string): string {
	return `{ timeout -k 10 ${HANG} ${command} || { s=$?; [ $s -ne 124 ] || echo "${what} ran past ${HANG} s: taken as hung" >&2; exit $s; }; }`;
}

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

/** Where a machine keeps what the compiler names: its sysroot, `rustc -vV`, cargo's home, this folder. */
interface Machine {
	sysroot: string;
	version: string;
	cargo: string;
	root: string;
}

/** This machine, as the compiler sees it from this folder. */
async function here(): Promise<Machine> {
	return {
		sysroot: (await $`rustc --print sysroot`.cwd(ROOT).text()).trim(),
		version: await $`rustc -vV`.cwd(ROOT).text(),
		cargo: process.env.CARGO_HOME ?? `${homedir()}/.cargo`,
		root: ROOT
	};
}

/**
 * The compiler's flags that write a machine's paths as one name: this
 * folder as `/tkn-32`, cargo's home as `/cargo`, and the standard library's
 * sources, which a machine with them installed would name in its own
 * place, as the compiler's release names them. Encoded as cargo takes a
 * list, so a path with a space stays one.
 */
function remapped(machine: Machine): string {
	const commit = machine.version.match(/^commit-hash: (\w+)$/m)?.[1];
	if (!commit) throw new Error('rustc -vV names no commit');
	const prefixes: [string, string][] = [
		[`${machine.sysroot}/lib/rustlib/src/rust`, `/rustc/${commit}`],
		[machine.cargo, '/cargo'],
		[machine.root, '/tkn-32']
	];
	return prefixes.map(([from, to]) => `--remap-path-prefix=${from}=${to}`).join('\x1f');
}

/** The build of the emulator's packages, as cargo's arguments: on the committed lock, never one cargo would rewrite. */
const BUILD = ['build', '--release', '--locked', '--target', 'wasm32-unknown-unknown', ...PACKAGES.flatMap((name) => ['-p', name])];
/** The files the build gives, in `wasm32-unknown-unknown/release`. */
const RUNNER = 'runner.wasm';
const CHIP = 'chip_worklet.wasm';

/** The runner's and the chip's bytes, built on this machine. */
async function builtHere(): Promise<[Uint8Array, Uint8Array]> {
	const env = { ...process.env, CARGO_TARGET_DIR: TARGET, CARGO_ENCODED_RUSTFLAGS: remapped(await here()) };
	await $`cargo ${BUILD}`.cwd(ROOT).env(env);
	return [readFileSync(`${BUILT}/${RUNNER}`), readFileSync(`${BUILT}/${CHIP}`)];
}

/** Whether Docker answers: the pin's image runs only where it does. */
async function docker(): Promise<boolean> {
	return (await $`docker info`.quiet().nothrow()).exitCode === 0;
}

/**
 * The image the emulator is built in: `IMAGE` with what rust-toolchain.toml
 * adds to its compiler, the target and the components, made once and named
 * by what it is made of. A toolchain file naming another compiler than the
 * image's is refused: the pin's compiler is the image's.
 */
async function builder(): Promise<string> {
	const toolchain = readFileSync(`${ROOT}/rust-toolchain.toml`, 'utf8');
	const channel = toolchain.match(/^channel = "([^"]+)"$/m)?.[1];
	const imaged = IMAGE.match(/:([^@]+)@/)?.[1];
	if (channel !== imaged) throw new Error(`rust-toolchain.toml names ${channel}, the pin's image ${imaged}: pin the image of that compiler`);
	const install = bounded('rustup toolchain install', 'the toolchain install');
	const dockerfile = `FROM --platform=${PLATFORM} ${IMAGE}\nCOPY rust-toolchain.toml /toolchain/\nRUN cd /toolchain && ${install}\n`;
	const tag = `tkn-32-emulator:${sha256(new TextEncoder().encode(dockerfile + toolchain)).slice(0, 16)}`;
	if ((await $`docker image inspect ${tag}`.quiet().nothrow()).exitCode === 0) return tag;
	const context = mkdtempSync(`${tmpdir()}/tkn-32-emulator-`);
	try {
		writeFileSync(`${context}/rust-toolchain.toml`, toolchain);
		writeFileSync(`${context}/Dockerfile`, dockerfile);
		await $`docker build --platform ${PLATFORM} -t ${tag} ${context}`;
	} finally {
		rmSync(context, { recursive: true, force: true });
	}
	return tag;
}

/**
 * The runner's and the chip's bytes, built in the pin's image: the
 * console's files, every one git keeps or would, go in on the container's
 * input and the two built files come out on its output, so no folder of
 * this machine is shared with it. Cargo's downloads and its target folder
 * are Docker volumes, kept between builds and shared by every checkout.
 * So one build at a time holds the target folder, from before its files
 * are unpacked until what it built is packed, and the console's own
 * packages are always built again: cargo judges them by the times of
 * their files, which another checkout's build, or the clock of a machine
 * that slept, makes look built. The registry's crates are judged by their
 * versions and stay built.
 */
async function builtInImage(): Promise<[Uint8Array, Uint8Array]> {
	const tag = await builder();
	const run = ['docker', 'run', '--rm', '--platform', PLATFORM];
	const asked = 'rustc --print sysroot && rustc -vV && printf "%s\\n" "$CARGO_HOME"';
	const said = (await $`${run} -w /toolchain ${tag} sh -c ${bounded(`sh -c '${asked}'`, 'asking the compiler')}`.text())
		.trim()
		.split('\n');
	const cargo = said[said.length - 1];
	const flags = remapped({ sysroot: said[0], version: said.slice(1, -1).join('\n'), cargo, root: INSIDE });
	const files = (await $`git ls-files -z --cached --others --exclude-standard`.cwd(ROOT).text())
		.split('\0')
		.filter((file) => file && existsSync(`${ROOT}/${file}`));
	const work = mkdtempSync(`${tmpdir()}/tkn-32-emulator-`);
	try {
		writeFileSync(`${work}/files`, files.join('\0'));
		mkdirSync(`${work}/built`);
		const inside = [
			`exec 9>${INSIDE_TARGET}/.emulator.lock`,
			`{ flock -w ${HANG} 9 || { echo "another build held the target folder past ${HANG} s" >&2; exit 1; }; }`,
			`mkdir -p ${INSIDE} && tar -x -f - -C ${INSIDE} && cd ${INSIDE}`,
			`cargo clean --release --target wasm32-unknown-unknown --workspace >&2`,
			bounded(`cargo ${BUILD.join(' ')} >&2`, 'the build'),
			`tar -c -f - -C ${INSIDE_TARGET}/wasm32-unknown-unknown/release ${RUNNER} ${CHIP}`
		].join(' && ');
		const volumes = ['-v', `tkn-32-emulator-cargo:${cargo}/registry`, '-v', `tkn-32-emulator-target:${INSIDE_TARGET}`];
		const env = ['-e', `CARGO_TARGET_DIR=${INSIDE_TARGET}`, '-e', `CARGO_ENCODED_RUSTFLAGS=${flags}`];
		// ustar carries no extended attributes, and COPYFILE_DISABLE keeps
		// macOS's tar from adding its own files beside each one. The
		// container ends the pipeline, so a build that fails fails it.
		await $`tar -c -f - --format ustar -C ${ROOT} --null -T ${work}/files | ${run} -i ${volumes} ${env} ${tag} sh -c ${inside} > ${work}/built.tar`.env(
			{ ...process.env, COPYFILE_DISABLE: '1' }
		);
		await $`tar -x -f ${work}/built.tar -C ${work}/built`;
		return [readFileSync(`${work}/built/${RUNNER}`), readFileSync(`${work}/built/${CHIP}`)];
	} finally {
		rmSync(work, { recursive: true, force: true });
	}
}

/** A file of an emulator: its hash, then what it is. */
const BUILT_FILE = /^[0-9a-f]{64}\.(wasm|js|json)$/;

/** Where the emulator's packages are built: on this machine, or in the pin's image. */
export type Where = 'here' | 'image';

/**
 * Builds the emulator into `out`, in a cargo build of its own: built beside
 * another package, the features they share would be unified, and the
 * emulator would be what that build made it, not the one every host runs.
 * Writes its files and `emulator.json`, and removes the files of an older
 * build the manifest no longer names, so the folder is one emulator. Built
 * here, it says when its files are not those of the pin the host above
 * this console keeps.
 */
export async function buildEmulator(out: string, where: Where = 'here'): Promise<Manifest> {
	const [runner, chip] = where === 'image' ? await builtInImage() : await builtHere();
	mkdirSync(out, { recursive: true });
	const read = async (file: string) => new Uint8Array(await Bun.file(file).arrayBuffer());
	const { hosted } = await import(`${ROOT}/crates/runner/host.js`);
	const emulator = await hosted(new WebAssembly.Module(runner));
	const manifest: Omit<Manifest, 'cases'> = {
		contract: emulator.contract,
		engine: emulator.engine,
		runner: await keep(out, runner, 'wasm'),
		host: await keep(out, await read(`${ROOT}/crates/runner/host.js`), 'js'),
		chip: await keep(out, chip, 'wasm'),
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
	const pin = pinAbove();
	if (where === 'here' && pin && pin !== out) {
		const differ = compare(manifestIn(pin), whole).map(({ part }) => part);
		if (differ.length > 0) {
			const machine = `${process.platform}/${process.arch}`;
			console.log(`this machine's build (${machine}), for work: its ${differ.join(', ')} are not the pin's in ${pin}, which only the pin's image builds (bun run emulator --pin)`);
		}
	}
	return whole;
}

/** The manifest of the emulator in `folder`. */
function manifestIn(folder: string): Manifest {
	return JSON.parse(readFileSync(`${folder}/emulator.json`, 'utf8'));
}

/**
 * The emulator a host keeps above this console, as the console's tests
 * find it: an `emulator/` with its manifest in a folder this one is in.
 */
function pinAbove(): string | undefined {
	for (let folder = dirname(ROOT); folder !== dirname(folder); folder = dirname(folder)) {
		if (existsSync(`${folder}/emulator/emulator.json`)) return `${folder}/emulator`;
	}
	return undefined;
}

/** Each part of the manifest `built` gives otherwise than `pinned`. */
function compare(pinned: Manifest, built: Manifest): { part: keyof Manifest; said: string }[] {
	return (Object.keys(built) as (keyof Manifest)[])
		.filter((part) => JSON.stringify(built[part]) !== JSON.stringify(pinned[part]))
		.map((part) => ({ part, said: `${part}: pinned ${JSON.stringify(pinned[part])}, built ${JSON.stringify(built[part])}` }));
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
 * Builds the emulator in the pin's image, in a folder of its own, and
 * compares it with the one at `pinned`: what differs, empty when the pin
 * is what this source builds.
 */
export async function differences(pinned: string): Promise<string[]> {
	const built = await buildEmulator(`${TARGET}/same`, 'image');
	return compare(manifestIn(pinned), built).map(({ said }) => said);
}

if (import.meta.main) {
	const [flag, folder] = process.argv.slice(2);
	if (flag === '--same' || flag === '--pin') {
		if (!folder) throw new Error(`bun run emulator ${flag} <folder>`);
		if (!(await docker())) throw new Error(`Docker does not answer: ${flag} builds in the pin's image (${PLATFORM})`);
	}
	if (flag === '--same') {
		const pinned = resolve(folder);
		const differ = await differences(pinned);
		if (differ.length > 0) {
			console.error(`the emulator at ${pinned} is not what this source builds:\n  ${differ.join('\n  ')}`);
			process.exit(1);
		}
		console.log(`the emulator at ${pinned} is what this source builds in the pin's image`);
	} else if (flag === '--pin') {
		const out = resolve(folder);
		await buildEmulator(out, 'image');
		console.log(`the emulator, built in the pin's image -> ${out}`);
	} else {
		const out = flag ? resolve(flag) : EMULATOR;
		await buildEmulator(out);
		console.log(`the emulator -> ${out}`);
	}
}

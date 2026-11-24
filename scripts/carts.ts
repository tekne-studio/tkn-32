// The carts the console's commands take: its own examples, by name, and a
// game of one's own, by its folder. Each is built in the cargo workspace it
// is in, which keeps it in its own target/.
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, relative, resolve } from 'node:path';
import { $ } from 'bun';
import { ROOT } from './lib';

/** Where a workspace's cargo leaves a cart built for WebAssembly. */
export const BUILT = 'target/wasm32-unknown-unknown/release';

/** A cart crate: its package's name, the file cargo builds, the folder it is in, and the workspace that builds it. */
export interface Cart {
	name: string;
	/** The cdylib's name, as cargo names the `.wasm` it writes. */
	lib: string;
	/** How the console's commands take it: its name, under examples/; its folder from the console's, elsewhere. */
	called: string;
	dir: string;
	root: string;
}

/** The packages of the cargo workspace at `root` that build a cdylib, as cargo itself reads their manifests. */
async function cdylibs(root: string): Promise<Cart[]> {
	interface Package {
		name: string;
		manifest_path: string;
		targets: { name: string; kind: string[] }[];
	}
	const { packages }: { packages: Package[] } = await $`cargo metadata --no-deps --format-version 1`.cwd(root).quiet().json();
	return packages.flatMap(({ name, manifest_path, targets }) => {
		const lib = targets.find(({ kind }) => kind.includes('cdylib'));
		const dir = dirname(manifest_path);
		return lib ? [{ name, lib: lib.name.replaceAll('-', '_'), called: relative(ROOT, dir), dir, root }] : [];
	});
}

/** Every cart crate under examples/. */
export async function examples(): Promise<Cart[]> {
	return (await cdylibs(ROOT))
		.filter(({ dir }) => dirname(dir) === `${ROOT}/examples`)
		.map((cart) => ({ ...cart, called: cart.name }))
		.sort((a, b) => basename(a.dir).localeCompare(basename(b.dir)));
}

/** The cart crate in folder `path`, built in the workspace it is in: the nearest folder above with a Cargo.lock. */
export async function cartIn(path: string): Promise<Cart> {
	const dir = resolve(path);
	if (!existsSync(`${dir}/Cargo.toml`)) throw new Error(`${dir}: no Cargo.toml, so no cart`);
	let root = dir;
	while (!existsSync(`${root}/Cargo.lock`)) {
		if (dirname(root) === root) throw new Error(`${dir}: in no cargo workspace`);
		root = dirname(root);
	}
	const cart = (await cdylibs(root)).find((cart) => cart.dir === dir);
	if (!cart) throw new Error(`${dir}: no package with a cdylib, so no cart`);
	return cart;
}

/** The cart `asked` names: a folder when it has a `/`, else one under examples/. */
export async function cartNamed(asked: string): Promise<Cart> {
	if (asked.includes('/')) return cartIn(asked);
	const cart = (await examples()).find(({ name }) => name === asked);
	if (!cart) throw new Error(`no cart ${asked} under examples/; name a cart elsewhere by its folder`);
	return cart;
}

/** Builds `carts` in their workspaces, each workspace once; `quiet` keeps cargo's progress out. */
export async function build(carts: Cart[], quiet = false) {
	for (const root of new Set(carts.map(({ root }) => root))) {
		const packages = carts.filter((cart) => cart.root === root).flatMap(({ name }) => ['-p', name]);
		// Joined to the rustflags the workspace's own config gives, never in their place.
		const flags = `target.wasm32-unknown-unknown.rustflags=${JSON.stringify(await remapped(root))}`;
		await $`cargo build --release ${quiet ? ['-q'] : []} --target wasm32-unknown-unknown --config ${flags} ${packages}`.cwd(root);
	}
	// RUSTFLAGS in the environment replaces every rustflags cargo is given, these too.
	for (const cart of carts) {
		const bytes = Buffer.from(await Bun.file(built(cart)).arrayBuffer());
		if (bytes.includes(homedir())) {
			throw new Error(`${built(cart)} names ${homedir()}: build it with RUSTFLAGS unset, so its paths are written short`);
		}
	}
}

/**
 * What keeps the builder's machine out of a cart: rustc writes a source's path where a panic
 * points, whole for a crate outside the workspace (the console's, to a game of one's own) and
 * for the standard library's sources once rust-src is installed. Each prefix is written as a
 * short name instead, and a later one wins over an earlier one that holds it.
 */
async function remapped(root: string): Promise<string[]> {
	const sysroot = (await $`rustc --print sysroot`.cwd(root).quiet().text()).trim();
	const commit = /^commit-hash: (\S+)$/m.exec(await $`rustc -vV`.cwd(root).quiet().text())?.[1];
	if (!commit) throw new Error('rustc -vV names no commit-hash');
	const prefixes: [string, string][] = [
		[homedir(), '~'],
		[process.env.CARGO_HOME ?? `${homedir()}/.cargo`, '/cargo'],
		// Where the standard library's sources are said to be when rust-src is not installed.
		[`${sysroot}/lib/rustlib/src/rust`, `/rustc/${commit}`],
		[ROOT, 'tkn-32'],
		[root, '.']
	];
	return prefixes.map(([from, to]) => `--remap-path-prefix=${from}=${to}`);
}

/** Where a built cart lies in its workspace. */
export const built = ({ root, lib }: Cart) => `${root}/${BUILT}/${lib}.wasm`;

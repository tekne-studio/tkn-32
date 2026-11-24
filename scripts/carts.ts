// The carts the console's commands take: its own examples, by name, and a
// game of one's own, by its folder. Each is built in the cargo workspace it
// is in, which keeps it in its own target/.
import { existsSync } from 'node:fs';
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
		await $`cargo build --release ${quiet ? ['-q'] : []} --target wasm32-unknown-unknown ${packages}`.cwd(root);
	}
}

/** Where a built cart lies in its workspace. */
export const built = ({ root, lib }: Cart) => `${root}/${BUILT}/${lib}.wasm`;

// A game of one's own, begun from the guide's dot in a folder outside the
// console: a cargo workspace of its own that depends on the console's crates
// by path, its lock begun at the versions the console locks and tests. The
// game is its maker's, under the licence they choose; examples/ holds only
// the console's own.
//   bun run new ../mygame
import { cpSync, existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve, sep } from 'node:path';
import { $ } from 'bun';
import { ROOT } from './lib';

// Real paths, with no link on the way: a path written relative from a link's
// name leads somewhere else once cargo follows the link.
const CONSOLE = realpathSync(ROOT);
const DOT = `${CONSOLE}/examples/dot`;

interface Workspace {
	workspace: {
		package: Record<string, string | boolean>;
		dependencies: Record<string, { path?: string }>;
	};
}

/** A TOML value as a manifest writes it. */
const toml = (value: string | boolean) => (typeof value === 'string' ? JSON.stringify(value) : String(value));

/**
 * Dot's manifest for a game named `name` in `dir`, standing alone: each field
 * dot takes from the console's workspace written out, its licence left to
 * its maker, each of the console's crates by its path from `dir`, and the
 * profiles a cart is built and tested with.
 */
function manifest(name: string, dir: string): string {
	const text = readFileSync(`${CONSOLE}/Cargo.toml`, 'utf8');
	const { workspace } = Bun.TOML.parse(text) as Workspace;
	const lines = readFileSync(`${DOT}/Cargo.toml`, 'utf8')
		.split('\n')
		.flatMap((line) => {
			if (line.startsWith('name = ')) return [`name = ${toml(name)}`];
			if (line.startsWith('description = ')) return [`description = ${toml(`${name}, a game for TKN-32.`)}`];
			const inherited = line.match(/^([a-z-]+)\.workspace = true$/);
			if (!inherited) return [line];
			const [, key] = inherited;
			if (key === 'license') return ['# license: its maker chooses it'];
			if (key in workspace.package) return [`${key} = ${toml(workspace.package[key])}`];
			const path = workspace.dependencies[key]?.path;
			if (!path) throw new Error(`dot takes ${key} from the workspace, which has no path for it`);
			return [`${key} = { path = ${toml(relative(dir, `${CONSOLE}/${path}`))} }`];
		});
	// The console's profiles, as text: every table from the first of them on,
	// each with the comment above it.
	const profiles = text.slice(text.lastIndexOf('\n\n', text.indexOf('\n[profile.')) + 2);
	return `${lines.join('\n').trimEnd()}\n\n# A workspace of its own, with its own lock and target/.\n[workspace]\n\n${profiles}`;
}

/** `path` with every link on the way followed, the folders not made yet as named. */
function real(path: string): string {
	if (existsSync(path)) return realpathSync(path);
	return dirname(path) === path ? path : join(real(dirname(path)), basename(path));
}

/** Begins a game named by its folder, `folder`, from dot, and says where it is. */
export async function newGame(folder: string): Promise<string> {
	const dir = real(resolve(folder));
	const name = basename(dir);
	if (!/^[a-z][a-z0-9_-]*$/.test(name)) throw new Error(`${name}: a game's folder is its crate's name, lower case`);
	if (!relative(CONSOLE, dir).startsWith(`..${sep}`)) throw new Error(`${folder}: a game of one's own lives outside the console`);
	if (existsSync(dir)) throw new Error(`${dir}: already there`);
	mkdirSync(dirname(dir), { recursive: true });
	cpSync(DOT, dir, { recursive: true, filter: (from) => basename(from) !== 'Cargo.toml' && basename(from) !== 'target' });
	writeFileSync(`${dir}/Cargo.toml`, manifest(name, dir));
	writeFileSync(`${dir}/.gitignore`, 'target/\n');
	// Its lock begins as the console's, so the game builds and tests on the
	// versions the console does, until its maker moves one.
	cpSync(`${CONSOLE}/Cargo.lock`, `${dir}/Cargo.lock`);
	await $`cargo update --workspace --quiet`.cwd(dir);
	return dir;
}

if (import.meta.main) {
	const [folder] = process.argv.slice(2);
	if (!folder) throw new Error('bun run new <folder outside the console>');
	const dir = await newGame(folder);
	const at = relative(process.cwd(), dir);
	const next = [
		[`bun run workbench ${at}`, 'plays it, rebuilt as you save'],
		[`bun run cart ${at}`, 'runs it with no window, a strip of PNGs'],
		['cargo test', 'inside it: its test on every engine at hand']
	];
	const width = Math.max(...next.map(([command]) => command.length));
	console.log(`${basename(dir)}, begun from dot, in ${at}`);
	for (const [command, what] of next) console.log(`  ${command.padEnd(width)}  ${what}`);
}

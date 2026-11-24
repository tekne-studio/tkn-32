import { afterAll, expect, test } from 'bun:test';
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { $ } from 'bun';
import { ROOT } from './lib';
import { newGame } from './new';

const beside = mkdtempSync(`${tmpdir()}/tkn-32-new-`);
afterAll(() => rmSync(beside, { recursive: true, force: true }));

test("a game begun from dot stands alone, on the console's crates", async () => {
	const dir = await newGame(`${beside}/mygame`);
	const manifest = readFileSync(`${dir}/Cargo.toml`, 'utf8');
	expect(manifest).not.toContain('.workspace = true');
	expect(manifest).not.toMatch(/^license/m);
	// Cargo reads it as a workspace of its own, its crates the console's.
	const { packages, workspace_root } = await $`cargo metadata --format-version 1 --no-deps`.cwd(dir).quiet().json();
	expect(workspace_root).toBe(dir);
	const [game] = packages;
	expect(game.name).toBe('mygame');
	for (const dependency of game.dependencies) expect(dependency.path).toBe(`${realpathSync(ROOT)}/crates/${dependency.name}`);
	// Its test runs it through the console's test support, a crate as the contract is.
	expect(game.dependencies).toContainEqual(expect.objectContaining({ name: 'cartridge-test', kind: 'dev' }));
});

test("a game of one's own lives outside the console, through a link too", async () => {
	symlinkSync(`${ROOT}/examples`, `${beside}/examples`);
	symlinkSync(ROOT, `${beside}/console`);
	const inside = [`${ROOT}/examples/mine`, `${beside}/examples/mine`, `${beside}/console/games/mine`, `${ROOT}/..x/mine`];
	for (const folder of inside) await expect(newGame(folder)).rejects.toThrow('outside the console');
	for (const made of ['examples/mine', 'games', '..x']) expect(existsSync(`${ROOT}/${made}`)).toBe(false);
});

test("a folder beside the console whose name begins with two dots is outside it", async () => {
	const dir = await newGame(`${beside}/..x/mygame`);
	expect(dir).toBe(`${realpathSync(beside)}/..x/mygame`);
});

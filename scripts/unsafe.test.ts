import { afterAll, expect, test } from 'bun:test';
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { ROOT } from './lib';
import { unsafeProblems } from './unsafe';

const copy = mkdtempSync(`${tmpdir()}/tkn-32-unsafe-`);
afterAll(() => rmSync(copy, { recursive: true, force: true }));

test('the console writes `unsafe` in its ABI files alone', () => {
	expect(unsafeProblems(ROOT)).toEqual([]);
});

test('an `unsafe` elsewhere, a crate off the lints and a cart that allows it are each said', () => {
	for (const dir of ['crates', 'examples']) {
		cpSync(`${ROOT}/${dir}`, `${copy}/${dir}`, { recursive: true, filter: (from) => !from.includes('/target') });
	}
	writeFileSync(`${copy}/crates/chip/src/sneaky.rs`, '#[allow(unsafe_code)]\nfn f() {}\n');
	mkdirSync(`${copy}/crates/loose/src`, { recursive: true });
	writeFileSync(`${copy}/crates/loose/Cargo.toml`, '[package]\nname = "loose"\n');
	writeFileSync(`${copy}/examples/dot/Cargo.toml`, '[package]\nname = "dot"\n');
	expect(unsafeProblems(copy)).toEqual([
		expect.stringMatching(/^crates\/chip\/src\/sneaky\.rs: 1 allow\(unsafe_code\)/),
		'crates/loose/Cargo.toml: no `[lints] workspace = true`, so `unsafe` is not forbidden there',
		'examples/dot/Cargo.toml: a cart denies unsafe_code; only the exports `cartridge::cart!` writes allow it'
	]);
});

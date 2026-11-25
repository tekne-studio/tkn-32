// The host's own calls, past what a page asks: a file too large, a window
// past the end, a step of every update there is, prepared bytes that
// preparing never gave, a script of every pad's buttons, a script too
// long, the codes its check answers, the pads a file plays. Each answers,
// and the emulator runs on. It runs the console's build of the emulator (`bun
// scripts/emulator.ts`, which `bun scripts/check.ts` runs first) on bun's
// WebAssembly.
import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { EMULATOR } from '../../scripts/emulator';

const MANIFEST = `${EMULATOR}/emulator.json`;
if (!existsSync(MANIFEST)) throw new Error(`no emulator built in ${EMULATOR}: bun scripts/emulator.ts`);
const { host, runner }: { host: string; runner: string } = JSON.parse(readFileSync(MANIFEST, 'utf8'));
const { hosted } = await import(`${EMULATOR}/${host}`);
const emulator = await hosted(new WebAssembly.Module(readFileSync(`${EMULATOR}/${runner}`)));

/** Dot, a cartridge as once built. */
const dot = new Uint8Array(readFileSync(`${import.meta.dir}/../headless/tests/kept/dot.wasm`));

/** A file of `length` bytes that throws when any of them is read: one the host must never copy. */
function unread(length: number): ArrayLike<number> {
	return new Proxy({ length } as ArrayLike<number>, {
		get(target, key) {
			if (key === 'length') return target.length;
			throw new Error(`byte ${String(key)} was read`);
		}
	});
}

/** Dot opened as `id`, running, and the emulator still whole after `work`. */
function runsAfter(work: () => void) {
	work();
	emulator.open(9, dot, 1, 0);
	expect(emulator.step(9, 3)).toBe(3);
	expect(emulator.stop(9)).toBeNull();
	emulator.close(9);
}

describe('the host', () => {
	test('refuses a file past the most by its size, its bytes never copied', () => {
		const size = emulator.most + 1024 * 1024;
		runsAfter(() => {
			expect(emulator.check(unread(size))).toBe('size');
			expect(emulator.prepare(unread(size))).toBeNull();
			emulator.open(1, unread(size), 1, 0);
			expect(emulator.stop(1)).toContain(`it is ${Math.ceil(size / 1024)} KB`);
			// A cover of a file too large is the stand-in, unread.
			expect(emulator.cover(unread(size), 'ab').said).toBeNull();
		});
	});

	test('reads a window past the end of memory as far as it goes', () => {
		runsAfter(() => {
			emulator.open(2, dot, 1, 0, true);
			emulator.step(2, 1);
			const size = emulator.size(2);
			const window = emulator.window(2, 100, 0xffffffff);
			expect(window.bytes.length).toBe(size - 100);
			expect(emulator.window(2, 0xffffffff, 0xffffffff).bytes.length).toBe(0);
		});
	});

	test('steps no more updates a call than a cart keeps the tones of', () => {
		runsAfter(() => {
			emulator.open(3, dot, 1, 0);
			expect(emulator.step(3, 0xffffffff)).toBe(emulator.kept);
		});
	});

	test('gives a script past the longest no room, its words never read', () => {
		runsAfter(() => {
			emulator.open(4, dot, 1, 0);
			emulator.script(4, [1, 2]);
			for (const length of [emulator.longest + 1, 2 ** 32 + 1]) {
				expect(() => emulator.script(4, unread(length))).toThrow(`past the ${emulator.longest} one holds`);
			}
			// What it plays plays on, and a script of the longest is one.
			expect(emulator.scripted(4)).toBe(2);
			emulator.script(4, new Uint32Array(emulator.longest));
			expect(emulator.scripted(4)).toBe(2 + emulator.longest);
			emulator.close(4);
		});
	});

	test('plays a script of every pad, each update the whole number, and no other bit', () => {
		const { pads, padBits } = emulator;
		expect([pads, padBits]).toEqual([2, 8]);
		const a = emulator.buttons.get('a');
		const second = a << padBits;
		expect(emulator.shared.get('KeyK')).toBe(second);
		expect(emulator.shared.get('KeyZ')).toBe(a);
		expect(emulator.keys.get('KeyZ')).toBe(a);
		const all = [...emulator.buttons.values()].reduce((all, bit) => all | bit);
		const every = Array.from({ length: pads }, (_, pad) => all << (pad * padBits)).reduce((every, pad) => every | pad);
		runsAfter(() => {
			emulator.open(6, dot, 1, 0, true);
			emulator.script(6, [second, 0xffffffff, a | second]);
			// A room counted in words: the script is the updates given, none more or less.
			expect(emulator.scripted(6)).toBe(3);
			expect(emulator.step(6, 3)).toBe(3);
			const ran = emulator.ran(6, 0);
			expect([ran[0], ran[2], ran[4]]).toEqual([second, every, a | second]);
		});
	});

	test('names every code its check answers, once each', () => {
		const { cases }: { cases: string } = JSON.parse(readFileSync(MANIFEST, 'utf8'));
		const checked: { check: string }[] = JSON.parse(readFileSync(`${EMULATOR}/${cases}`, 'utf8'));
		const answered = new Set([emulator.check(unread(emulator.most + 1)), ...checked.map(({ check }) => check)]);
		answered.delete('');
		expect(new Set(emulator.refusals).size).toBe(emulator.refusals.length);
		expect(emulator.refusals).toEqual(expect.arrayContaining([...answered]));
	});

	test('seats the pads a file it keeps says, and nobody at a file it refuses', () => {
		const said = (pads: number) => [0, 16, 14, ...new TextEncoder().encode('cartridge.pads'), pads];
		expect(emulator.plays(dot)).toBe(1);
		expect(emulator.plays([...dot, ...said(emulator.pads)])).toBe(emulator.pads);
		expect(emulator.plays([...dot, ...said(emulator.pads + 1)])).toBeNull();
		// A module of nothing but the section: refused, no memory its own, whatever it says.
		const bare = [0x00, 0x61, 0x73, 0x6d, 1, 0, 0, 0, ...said(2)];
		expect(emulator.check(bare)).toBe('memory');
		expect(emulator.plays(bare)).toBeNull();
		expect(emulator.plays(unread(emulator.most + 1))).toBeNull();
	});

	test('stops prepared bytes preparing never gave as a failed load, alone', () => {
		runsAfter(() => {
			emulator.openPrepared(4, dot, 1, 0);
			expect(emulator.stop(4)).toContain('not prepared bytes');
			const prepared = emulator.prepare(dot);
			emulator.openPrepared(5, prepared, 1, 0);
			expect(emulator.step(5, 3)).toBe(3);
		});
	});
});

// The host's own calls, past what a page asks: a file too large, a window
// past the end, a step of every update there is, prepared bytes that
// preparing never gave. Each answers, and the emulator runs on. It runs the
// console's build of the emulator (`bun scripts/emulator.ts`, which `bun
// scripts/check.ts` runs first) on bun's WebAssembly.
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

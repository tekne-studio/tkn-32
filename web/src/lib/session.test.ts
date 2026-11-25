import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { read as readAs, write as writeAs, type Layout, type Session } from './session';

// The console's buttons and pads, from the manifest of the emulator the
// console's check builds first (`bun scripts/emulator.ts`, into
// target/emulator). Read as a file, never imported through scripts/: the
// check reads the page from a copy that holds no scripts/.
const built = JSON.parse(readFileSync(`${import.meta.dir}/../../../target/emulator/emulator.json`, 'utf8'));
const layout: Layout = {
	buttons: Object.entries(built.buttons),
	pads: built.pads,
	padBits: built.pad_bits,
	longest: built.longest
};
const read = (text: string) => readAs(text, layout);
const write = (session: Session, file: string) => writeAs(session, file, layout);

describe('a session', () => {
	test('comes back as it was written', () => {
		const buttons = [0, 0, 64, 64, 2 | 16, 2 | 16, 2, 0];
		const text = write({ cart: 'rocks', seed: 7, buttons, hash: '00ff' }, 'rocks.txt');
		expect(text).toContain('0:- 2:start 4:right+a 6:right 7:-');
		expect(text).toContain('# bun run cart rocks --seed 7 --frames 8 --buttons @rocks.txt');
		expect(read(text)).toEqual({ cart: 'rocks', seed: 7, hash: '00ff', buttons });
	});

	test('names the second pad by its number, as bun run cart reads it', () => {
		const second = (bits: number) => bits << layout.padBits;
		const buttons = [second(64), 1 | second(1 | 16), 0];
		const text = write({ cart: 'dot', seed: 1, buttons }, 'dot.txt');
		expect(text).toContain('0:2.start 1:left+2.left+2.a 2:-');
		expect(read(text).buttons).toEqual(buttons);
	});

	test('reads the script bun run cart takes, each item held to the next', () => {
		const { buttons, seed } = read('30:start 34:- # go\n36:left+a');
		expect(seed).toBeUndefined();
		expect(buttons.length).toBe(37);
		expect(buttons[29]).toBe(0);
		expect(buttons[30]).toBe(64);
		expect(buttons[33]).toBe(64);
		expect(buttons[34]).toBe(0);
		expect(buttons[36]).toBe(1 | 16);
	});

	test('refuses what is no script', () => {
		expect(() => read('3:jump')).toThrow('no button jump');
		expect(() => read(`0:${layout.pads + 1}.a`)).toThrow('no button');
		expect(() => read('0:1.a')).toThrow('no button 1.a');
		expect(() => read('5:a 2:b')).toThrow('in order');
		expect(() => read('a')).toThrow('UPDATE:NAMES');
	});

	test('refuses a length past the longest script, said or implied', () => {
		expect(() => read('# updates 1000000000\n0:a')).toThrow('at most');
		expect(() => read(`${layout.longest}:a`)).toThrow('at most');
		expect(read(`${layout.longest - 1}:a`).buttons.length).toBe(layout.longest);
	});
});

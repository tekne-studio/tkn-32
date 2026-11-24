import { describe, expect, test } from 'bun:test';
import { LONGEST, read, write } from './session';

describe('a session', () => {
	test('comes back as it was written', () => {
		const buttons = [0, 0, 64, 64, 2 | 16, 2 | 16, 2, 0];
		const text = write({ cart: 'rocks', seed: 7, buttons, hash: '00ff' }, 'rocks.txt');
		expect(text).toContain('0:- 2:start 4:right+a 6:right 7:-');
		expect(text).toContain('# bun run cart rocks --seed 7 --frames 8 --buttons @rocks.txt');
		expect(read(text)).toEqual({ cart: 'rocks', seed: 7, hash: '00ff', buttons });
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
		expect(() => read('5:a 2:b')).toThrow('in order');
		expect(() => read('a')).toThrow('UPDATE:NAMES');
	});

	test('refuses a length past a day of play, said or implied', () => {
		expect(() => read('# updates 1000000000\n0:a')).toThrow('at most');
		expect(() => read(`${LONGEST}:a`)).toThrow('at most');
		expect(read(`${LONGEST - 1}:a`).buttons.length).toBe(LONGEST);
	});
});

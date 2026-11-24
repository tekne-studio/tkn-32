import { describe, expect, test } from 'bun:test';
import { line, parse } from './tone';

describe('a tone line', () => {
	test('comes back as it was written', () => {
		const slide = { channel: 0, hz: 165, ms: 346, wave: 1, toHz: 82, loud: 255, toLoud: 40 };
		expect(line(slide)).toBe('tone_to(0, 165, 346, wave::TRIANGLE, 82, 255, 40);');
		expect(parse(line(slide))).toEqual(slide);
		const steady = { channel: 2, hz: 418, ms: 40, wave: 1, toHz: 418, loud: 255, toLoud: 255 };
		expect(line(steady)).toBe('tone(2, 418, 40, wave::TRIANGLE);');
		expect(parse(line(steady))).toEqual(steady);
	});

	test('reads a wave by name or by number', () => {
		expect(parse('tone_to(3, 1500, 300, cartridge::wave::NOISE, 3000, 50, 200)')?.wave).toBe(3);
		expect(parse('tone(1, 220, 100, 2)')?.wave).toBe(2);
	});

	test('refuses what is no tone', () => {
		expect(parse('tone(1, 220)')).toBeNull();
		expect(parse('tone(1, hz, 100, 2)')).toBeNull();
		expect(parse('play(1, 220, 100, 2)')).toBeNull();
	});
});

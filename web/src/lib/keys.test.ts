import { expect, test } from 'bun:test';
import { keysOf } from './keys';

test('reads each key as a person does, its side only when the other side is not the same', () => {
	const table: [string, number][] = [
		['ArrowLeft', 1],
		['KeyA', 1],
		['Digit1', 64],
		['Numpad4', 1],
		['ShiftLeft', 32],
		['ShiftRight', 32],
		['ControlLeft', 16],
		['ControlRight', 2]
	];
	expect(keysOf(table, 1)).toBe('← or A or 4');
	expect(keysOf(table, 64)).toBe('1');
	expect(keysOf(table, 32)).toBe('Shift');
	expect(keysOf(table, 16)).toBe('left Control');
	expect(keysOf(table, 8)).toBe('');
});

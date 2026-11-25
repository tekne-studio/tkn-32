/**
 * How a keyboard's keys read to a person, each by the name a page's keyboard
 * events give it (`KeyboardEvent.code`): on the workbench's hints and in the
 * console's docs, whose keys tables the docs' check writes with it.
 */

const ARROWS: Record<string, string> = { ArrowLeft: '←', ArrowRight: '→', ArrowUp: '↑', ArrowDown: '↓' };

/**
 * The keys of `table` that play `bit`, as a person reads them, joined by "or": `KeyZ` as Z, `Digit1` as 1, an arrow
 * as one, a numpad's as the key it doubles, and a key of two sides by its side only when its other side does
 * not play the same bit (`left Shift`).
 */
export function keysOf(table: Iterable<readonly [string, number]>, bit: number): string {
	const codes = [...table].filter(([, played]) => played === bit).map(([code]) => code);
	return [...new Set(codes.map((code) => said(code, codes)))].join(' or ');
}

function said(code: string, codes: string[]): string {
	const arrow = ARROWS[code];
	if (arrow) return arrow;
	const bare = code.replace(/^(Key|Digit|Numpad)/, '');
	const [, key, side] = /^(.+)(Left|Right)$/.exec(bare) ?? [];
	if (!key || !side) return bare;
	const other = side === 'Left' ? 'Right' : 'Left';
	return codes.includes(`${key}${other}`) ? key : `${side.toLowerCase()} ${key}`;
}

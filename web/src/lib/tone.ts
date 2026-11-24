/**
 * A tone in `env.tone`'s words, as the page draws, edits and plays it, and
 * the line a game writes to play it (`cartridge::tone` and `tone_to`).
 */

/** The waves, by their `cartridge::wave` names, in their numbers' order. */
export const WAVES = ['SQUARE', 'TRIANGLE', 'SAW', 'NOISE'] as const;
export const CHANNELS = 4;
/** The words a tone is. */
export const WORDS = 7;
/** The most a loudness is. */
export const LOUDEST = 255;
/** The pitches and lengths the chip plays within (`crates/chip`). */
export const PITCHES: [number, number] = [20, 12_000];
export const LENGTHS: [number, number] = [10, 2_000];

export interface Tone {
	channel: number;
	hz: number;
	ms: number;
	wave: number;
	toHz: number;
	loud: number;
	toLoud: number;
}

/** A tone played, by the update that played it. */
export interface Played {
	update: number;
	tone: Tone;
}

/** The tone in seven words, `channel hz ms wave to_hz loud to_loud`. */
export function toneOf(words: ArrayLike<number>): Tone {
	const [channel, hz, ms, wave, toHz, loud, toLoud] = Array.from(words);
	return { channel, hz, ms, wave, toHz, loud, toLoud };
}

export function wordsOf({ channel, hz, ms, wave, toHz, loud, toLoud }: Tone): number[] {
	return [channel, hz, ms, wave, toHz, loud, toLoud];
}

/** Whether the tone silences its channel instead of sounding. */
export const silences = (tone: Tone) => tone.hz === 0 || (tone.loud === 0 && tone.toLoud === 0);

/** The line a game writes to play the tone: `tone` when it stays, else `tone_to`. */
export function line(tone: Tone): string {
	const { channel, hz, ms, toHz, loud, toLoud } = tone;
	const wave = `wave::${WAVES[tone.wave] ?? tone.wave}`;
	const steady = hz === toHz && loud === LOUDEST && toLoud === LOUDEST;
	return steady
		? `tone(${channel}, ${hz}, ${ms}, ${wave});`
		: `tone_to(${channel}, ${hz}, ${ms}, ${wave}, ${toHz}, ${loud}, ${toLoud});`;
}

/**
 * A tone from the line a game writes: `tone(channel, hz, ms, wave)` or
 * `tone_to(channel, hz, ms, wave, to_hz, loud, to_loud)`, each a whole
 * number but the wave, which may be named (`wave::SAW`, `SAW`). Null when
 * the line is no such call.
 */
export function parse(text: string): Tone | null {
	const call = /\b(tone|tone_to)\s*\(([^)]*)\)/.exec(text);
	if (!call) return null;
	const args = call[2].split(',').map((arg) => arg.trim());
	if (args.length !== (call[1] === 'tone' ? 4 : 7)) return null;
	const named = WAVES.indexOf(args[3].replace(/^(cartridge::)?wave::/, '') as (typeof WAVES)[number]);
	const numbers = args.map((arg, at) => (at === 3 && named >= 0 ? named : /^\d+$/.test(arg) ? Number(arg) : NaN));
	if (numbers.some(Number.isNaN)) return null;
	const [channel, hz, ms, wave, toHz = hz, loud = LOUDEST, toLoud = LOUDEST] = numbers;
	return { channel, hz, ms, wave, toHz, loud, toLoud };
}

import type { Covered } from '../../../crates/runner/host.js';

export type { Covered };

/** What the page asks the bench's worker. */
export type Asked =
	/**
	 * A game begins, watched: `game` stamps every answer until the next one.
	 * With `script`, those buttons play from the first update; with `seek`,
	 * the game runs at once to that update, then holds. Its clock runs at
	 * `pace`, 0 holding it.
	 */
	| { open: ArrayBuffer; seed: number; game: number; script: number[] | null; seek: number | null; pace: number }
	/** The page drew a frame: the game's clock runs what is due. */
	| { tick: true }
	| { pace: number }
	/** The buttons the keys hold now. */
	| { press: number }
	/** That many updates now, outside the clock. */
	| { step: number }
	/** The rest of the script, at once. */
	| { toEnd: true }
	/** The stretch of memory the page shows. */
	| { window: { at: number; len: number } };

/** The console as the emulator says it, once it is ready: its numbers and keys. */
export interface Console {
	width: number;
	height: number;
	/** Updates a second, at a pace of 1. */
	ticks: number;
	/** The units an update may spend. */
	budget: number;
	/** The bytes a block of memory's map stands for. */
	block: number;
	/** Each key one person plays by, by its `KeyboardEvent.code`, and its button's bit: the first pad's. */
	keys: [string, number][];
	/** Each key two people sharing the keyboard play by, a half each, and its button's bit in either pad. */
	shared: [string, number][];
	/** Each button by its name, in the order of its bit in the first pad's byte. */
	buttons: [string, number][];
	/** The pads the buttons' number carries, a byte each from its lowest. */
	pads: number;
	/** How far each pad's byte lies above the one before it, in bits. */
	padBits: number;
	/** The most updates a script holds, the longest session the console keeps. */
	longest: number;
}

/**
 * What the worker tells the page: the console once the emulator is ready,
 * why it could not be, and an answer to each thing asked. A tick is
 * answered each time, with what it ran or with nothing.
 */
export type Told = { ready: Console } | { failed: string } | { ticked: true } | Seen;

/** What the worker answers after an open, or after updates ran. */
export interface Seen {
	/** The game this answers for, as its open stamped it. */
	game: number;
	/** Whether it answers a tick. */
	ticked: boolean;
	/** The file's cover, after an open alone. */
	cover?: Covered;
	/** The screen in colour, four bytes a pixel. */
	rgba: ArrayBuffer;
	/** The screen as indices into the palette, a byte a pixel. */
	indices: ArrayBuffer;
	/** The palette as the game wrote it, four bytes a colour. */
	palette: number[];
	updates: number;
	/** The frames hash so far, as `bun run cart` prints it. */
	hash: string;
	/** Why it stopped, as its screen says it. */
	stop: string | null;
	/** Each update run since the last answer: its buttons and the units it spent. */
	ran: { buttons: number; spent: number }[];
	/** Each update's tones, a tone's words one after another, a list for each of `ran`. */
	tones: number[][];
	/** What they played, the batch the chip takes; none while the game seeks. */
	sound: ArrayBuffer | null;
	/** How many updates of the script are left to play; null when none plays. */
	scripted: number | null;
	/** The update a seek runs to; null when none runs. */
	seeking: number | null;
	/** The bytes of memory it holds. */
	size: number;
	window: { at: number; bytes: ArrayBuffer; changes: ArrayBuffer };
	/** A byte a block of memory, 1 where the last updates changed it. */
	blocks: ArrayBuffer;
}

/**
 * A stretch of a cart's memory and what lies there, from the map its link
 * wrote (`scripts/linkmap.ts`): a cart built here has them, a file opened
 * has none.
 */
export interface Region {
	/** Where it starts, in bytes. */
	at: number;
	size: number;
	/** Its path, `rocks::CART`, or `stack`. */
	name: string;
}

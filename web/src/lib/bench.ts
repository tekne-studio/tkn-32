/** What the page asks the bench's worker. */
export type Asked =
  /** A game begins: `game` stamps every answer until the next one. */
  | { open: ArrayBuffer; seed: number; game: number }
  /** One update for each entry, with those buttons held. */
  | { run: number[] }
  /** The stretch of memory the page shows. */
  | { window: { at: number; len: number } };

/** What a cabinet shows of the file before anyone plays it (`cartridge-web`'s `cover`). */
export interface Covered {
  /** Its words, when the file carries a cover; without one, the picture is a cabinet's stand-in. */
  said: { title: string; maker: string; year: number } | null;
  /** The picture, four bytes a pixel. */
  rgba: ArrayBuffer;
}

/** What the worker answers after an open, a run or a new window. */
export interface Seen {
  /** The game this answers for, as its open stamped it. */
  game: number;
  /** The file's cover, after an open alone. */
  cover?: Covered;
  /** The screen in colour, four bytes a pixel. */
  rgba: ArrayBuffer;
  /** The screen as indices into the palette, a byte a pixel. */
  indices: ArrayBuffer;
  /** The palette, four bytes a colour, as many colours as the cart holds. */
  palette: number[];
  updates: number;
  /** The frames hash so far, as `bun run cart` prints it. */
  hash: string;
  /** Why it stopped, as its screen says it. */
  stop: string | null;
  /** The units each update of this run spent, in order. */
  spent: number[];
  /** The tones each update of this run played, seven words a tone. */
  tones: number[][];
  /** The bytes of memory it holds. */
  size: number;
  window: { at: number; bytes: ArrayBuffer; changes: ArrayBuffer };
  /** A byte a block of memory, 1 where the last update changed it. */
  blocks: ArrayBuffer;
  /** The bytes a block stands for. */
  block: number;
  /** The units an update may spend. */
  budget: number;
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

/**
 * The contract's buttons, a bit each, and the keys that press them: the
 * table a cabinet plays by.
 */
export const KEYS: Record<string, number> = {
  ArrowLeft: 1 << 0,
  KeyA: 1 << 0,
  ArrowRight: 1 << 1,
  KeyD: 1 << 1,
  ArrowUp: 1 << 2,
  KeyW: 1 << 2,
  ArrowDown: 1 << 3,
  KeyS: 1 << 3,
  Space: 1 << 4,
  KeyZ: 1 << 4,
  ShiftLeft: 1 << 5,
  ShiftRight: 1 << 5,
  KeyX: 1 << 5,
  Enter: 1 << 6,
};

/** The screen, in pixels. */
export const WIDTH = 160;
export const HEIGHT = 120;
/** Updates a second. */
export const TICKS = 60;

/**
 * The bench as the page drives it: the worker that holds the cartridge,
 * the clock that asks it for updates while it plays, the buttons of each
 * update, from the keys or played back, and what the last answer showed,
 * kept for the panels to read.
 *
 * Every update's buttons are recorded from the start, so the game played
 * so far is a session (`session.ts`): saved, it plays again exactly; and a
 * cart rebuilt is started again and run through it to the update it was at.
 */
import BenchWorker from "./bench.worker.ts?worker";
import {
  KEYS,
  TICKS,
  type Asked,
  type Covered,
  type Region,
  type Seen,
} from "./bench";
import type { Sound } from "./sound.svelte";
import { WORDS, toneOf, type Played } from "./tone";

/** How many updates the meter's strip keeps. */
export const KEPT = 600;
/** The most updates one frame of the page asks for, at speed 1. */
const CATCH_UP = 4;
/** The most updates one answer runs while the bench seeks. */
const SEEK = 600;
/** The stack a cart built as the guide says keeps below its data, when no map says. */
const STACK = 64 * 1024;

export class Bench {
  /** What the worker showed last. */
  seen = $state.raw<Seen | undefined>();
  /** What a cabinet shows of the file before anyone plays it. */
  cover = $state.raw<Covered | null>(null);
  /** The name the cartridge was opened by. */
  name = $state("");
  /** What lies where in its memory, by address, when its build mapped it. */
  regions = $state.raw<Region[]>([]);
  seed = $state(1);
  /** The seed the game running was started with. */
  started = $state(1);
  playing = $state(true);
  /** Updates a second, as a share of the contract's 60. */
  speed = $state(1);
  /** Units spent by the last [`KEPT`] updates, oldest first. */
  spent = $state.raw<number[]>([]);
  /** The tones the last [`KEPT`] updates played, in order. */
  played = $state.raw<Played[]>([]);
  /** How warm each block of memory is: 1 the update that changed it, fading. */
  heat = $state.raw<Float32Array>(new Float32Array());
  /** Where the memory shown starts. */
  at = $state(0);
  /** The bytes the memory panel shows. */
  readonly shown = 512;
  /** The buttons the keys hold now. */
  held = $state(0);
  /** Updates asked for since the start: the session's length. */
  sent = $state(0);
  /** Buttons being played back, one an update from the start, until a key takes over. */
  replay = $state.raw<number[] | null>(null);
  /** The update a seek runs to, as fast as the worker goes, before it pauses. */
  seeking = $state<number | null>(null);

  /** The buttons of every update asked for, from the start. */
  #recorded: number[] = [];
  /** Buttons pressed since the last update: a tap between two still counts. */
  #pressed = 0;
  #worker: Worker;
  #file: ArrayBuffer | undefined;
  #busy = false;
  /** The game begun last: an answer for an older one is dropped. */
  #game = 0;
  #owed = 0;
  #last = 0;
  #frame = 0;
  /** Whether the memory shown moves to the first block an update writes. */
  #follow = true;

  /** Where the tones of the updates run go; a seek runs silent. */
  readonly sound: Sound;

  constructor(sound: Sound) {
    this.sound = sound;
    this.#worker = new BenchWorker();
    this.#worker.onmessage = ({ data }: MessageEvent<Seen>) => this.#took(data);
    this.#frame = requestAnimationFrame((now) => this.#tick(now));
  }

  #ask(asked: Asked, transfer: Transferable[] = []) {
    this.#worker.postMessage(asked, transfer);
  }

  /** Opens `file` from its start, with the bench's seed. */
  open(file: ArrayBuffer, name: string, regions: Region[] = []) {
    this.#file = file;
    this.name = name;
    this.regions = regions;
    this.restart();
  }

  /**
   * Starts the cartridge again from its seed: the same seed, the same game.
   * With `replay`, those buttons are played back from the first update;
   * with `seek`, run through at once to that update, then paused.
   */
  restart(replay: number[] | null = null, seek: number | null = null) {
    if (!this.#file) return;
    this.spent = [];
    this.played = [];
    this.heat = new Float32Array();
    this.#recorded = [];
    this.sent = 0;
    this.replay = replay;
    this.seeking = seek && seek > 0 ? seek : null;
    this.#owed = 0;
    this.#follow = this.seeking === null;
    this.sound.hush();
    this.#busy = true;
    this.started = this.seed >>> 0;
    this.#ask({ open: this.#file.slice(0), seed: this.started, game: ++this.#game });
  }

  /** Opens a new build of the cartridge and runs it through the game played so far. */
  rebuilt(file: ArrayBuffer, regions: Region[] = []) {
    this.#file = file;
    this.regions = regions;
    this.restart([...this.#recorded], this.sent);
  }

  /** Plays the game played so far again from its start, at the bench's speed. */
  again() {
    this.restart([...this.#recorded]);
    this.playing = true;
  }

  /** Runs the rest of a replay at once. */
  toEnd() {
    if (this.replay) this.seeking = this.replay.length;
  }

  /** The game played so far: its seed and every update's buttons. */
  session() {
    return {
      cart: this.name,
      seed: this.started,
      buttons: [...this.#recorded],
      hash: this.seen?.hash,
    };
  }

  /** One update now, with the buttons held; it pauses the clock. */
  step() {
    this.playing = false;
    if (this.#busy || this.seen?.stop) return;
    this.#busy = true;
    this.#ask({ run: [this.#next()] });
  }

  /** Shows the memory from `at`, held inside it. */
  show(at: number) {
    this.#follow = false;
    const size = this.seen?.size ?? 0;
    const last = Math.max(0, Math.ceil(size / 16) * 16 - this.shown);
    this.at = Math.min(Math.max(0, Math.floor(at / 16) * 16), last);
    this.#ask({ window: { at: this.at, len: this.shown } });
  }

  /** A key went down: whether it was one of the game's. A key takes over from a replay. */
  press(code: string): boolean {
    const bit = KEYS[code];
    if (!bit) return false;
    if (this.replay && this.seeking === null) this.replay = null;
    this.held |= bit;
    this.#pressed |= bit;
    return true;
  }

  release(code: string): boolean {
    const bit = KEYS[code];
    if (!bit) return false;
    this.held &= ~bit;
    return true;
  }

  /** The buttons of the next update, recorded: played back, or the keys held and tapped. */
  #next(): number {
    let buttons: number;
    if (this.replay && this.sent < this.replay.length)
      buttons = this.replay[this.sent];
    else {
      if (this.replay) {
        this.replay = null;
        this.playing = false;
      }
      buttons = this.held | this.#pressed;
      this.#pressed = 0;
    }
    this.#recorded.push(buttons);
    this.sent++;
    return buttons;
  }

  #tick(now: number) {
    this.#frame = requestAnimationFrame((now) => this.#tick(now));
    const elapsed = this.#last ? Math.min(now - this.#last, 100) : 0;
    this.#last = now;
    if (this.#busy || !this.seen || this.seen.stop) {
      if (this.seen?.stop) this.#owed = 0;
      return;
    }
    if (this.seeking !== null) {
      const due = Math.min(this.seeking - this.sent, SEEK);
      if (due < 1) {
        this.seeking = null;
        this.playing = false;
        if (this.replay && this.sent >= this.replay.length) this.replay = null;
        return;
      }
      this.#busy = true;
      this.#ask({ run: Array.from({ length: due }, () => this.#next()) });
      return;
    }
    if (!this.playing) {
      this.#owed = 0;
      return;
    }
    this.#owed += (elapsed / 1000) * TICKS * this.speed;
    const due = Math.min(
      Math.floor(this.#owed),
      Math.ceil(CATCH_UP * this.speed),
    );
    if (due < 1) return;
    this.#owed -= due;
    this.#busy = true;
    this.#ask({ run: Array.from({ length: due }, () => this.#next()) });
  }

  #took(seen: Seen) {
    // The game restarted under it: its tones, units and updates are not
    // this game's, and the open it waits on is still to come.
    if (seen.game !== this.#game) return;
    this.#busy = false;
    this.seen = seen;
    if (seen.cover) this.cover = seen.cover;
    if (this.seeking === null)
      this.sound.play(seen.tones, TICKS * this.speed);
    if (seen.spent.length)
      this.spent = [...this.spent, ...seen.spent].slice(-KEPT);
    if (seen.tones.some((words) => words.length)) {
      const first = seen.updates - seen.tones.length;
      const played = this.played.filter(
        ({ update }) => update > seen.updates - KEPT,
      );
      seen.tones.forEach((words, i) => {
        for (let at = 0; at + WORDS <= words.length; at += WORDS) {
          played.push({
            update: first + i,
            tone: toneOf(words.slice(at, at + WORDS)),
          });
        }
      });
      this.played = played;
    }
    const blocks = new Uint8Array(seen.blocks);
    const heat = new Float32Array(blocks.length);
    for (let i = 0; i < blocks.length; i++) {
      heat[i] = blocks[i]
        ? 1
        : (this.heat[i] ?? 0) * (seen.spent.length ? 0.92 : 1);
    }
    this.heat = heat;
    // A Rust cart keeps its stack at the bottom (`cartridge_build::link`):
    // what it holds lies past it, when anything there moved.
    const stack =
      this.regions.find(({ name }) => name === "stack")?.size ?? STACK;
    const past = blocks.indexOf(1, Math.ceil(stack / seen.block));
    const first = past >= 0 ? past : blocks.indexOf(1);
    if (this.#follow && seen.spent.length && first >= 0) {
      this.#follow = false;
      this.show(first * seen.block);
    }
  }

  close() {
    cancelAnimationFrame(this.#frame);
    this.#worker.terminate();
  }
}

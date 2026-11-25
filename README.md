# TKN-32

A fantasy console: a small game is one WebAssembly file, a screen of
160 by 120 pixels in 32 colours, two pads of seven buttons and a
four-channel sound chip. The same file, the same seed and the same buttons give the same
frames on every runner: native, in a browser, in a test. The arcade
cabinets of [Planet](https://github.com/tekne-studio/planet) run it.

- Games are written in Rust with the `cartridge` crate; any language that
  builds WebAssembly can keep the contract below.
- `examples/dot` is the smallest whole game, the one a new game begins
  from; `examples/rocks` is a whole one.

## The contract

A game exports, every value an `i32`:

| Export | What it is |
|---|---|
| `cart_abi()` | the contract version it speaks: 1 |
| `init(seed)` | called once, before any update |
| `update(buttons)` | called 60 times a second with the buttons every pad holds |
| `frame()`, `palette()` | the addresses of its screen and its palette |
| `memory` | its own, never imported |

- **The screen:** 160 by 120 bytes, row by row from the top left, each an
  index into the palette.
- **The palette:** 32 colours, red, green, blue, alpha. The game picks all
  32 and may change them any update.
- **The one import:** `env.tone(channel, hz, ms, wave, to_hz, loud, to_loud)`,
  a tone whose pitch and loudness may slide.
- **The cover:** a picture of 160 by 120, a title, a maker and a year, in a
  custom section of the file. A host shows it without running the game.
- **The pads:** how many it plays, 1 to `button::PADS`, one byte in a custom section of
  its own (`cartridge::pads!`). A host seats as many people without running
  the game; a file without it plays one.
- No clock, no random source: the seed is the game's only chance.

## The limits

| Limit | Value |
|---|---|
| File | 256 KiB |
| Memory | 1 MiB |
| Work per update | 2 000 000 units, about one instruction each |
| Work to start | 20 000 000 units |
| Tones per update | 16 |

A runner prepares each file once: it refuses what breaks the contract and
writes a meter of work into the code, so every engine stops a runaway game
at the same instruction. A stopped game's screen says `STOPPED` and why.

## The buttons

A byte a pad from the lowest: the first pad's buttons in bits 0 to 6, the
second's in bits 8 to 14, in the same order. A runner sends no other bit,
and a game reads the pads it plays: `buttons.pad(2)` is the second pad's,
read by the same names. `held` with a name reads the first pad; `any`,
`pressed_since`, `==` and `.0` see every pad. One person plays the first
pad by every key; two sharing a keyboard play a half each.

<!-- generated:keys -->
| Button | Bits (pad 1, pad 2) | 1 person | 2 people: pad 1 | 2 people: pad 2 |
| --- | --- | --- | --- | --- |
| `LEFT` | 0, 8 | ← or A | A | ← |
| `RIGHT` | 1, 9 | → or D | D | → |
| `UP` | 2, 10 | ↑ or W | W | ↑ |
| `DOWN` | 3, 11 | ↓ or S | S | ↓ |
| `A` | 4, 12 | Space or Z | Space or Z | K |
| `B` | 5, 13 | Shift or X | left Shift or X | L |
| `START` | 6, 14 | Enter | 1 | Enter |
<!-- /generated:keys -->

## The chip

Four channels, each playing one tone at a time: square and saw band-limited
so a high note keeps its pitch, a plain triangle, and noise through a
low-pass whose edge is the tone's pitch. A new tone eases in without a
click. It uses only `libm` math, so its samples are the same bits on native
and WebAssembly; `chip-worklet` runs it in a browser's AudioWorklet.

## Make a game in five minutes

Needs Rust at the version `rust-toolchain.toml` names (rustup takes it, and
the `wasm32-unknown-unknown` target with it) and Bun. Once after a clone:

```sh
bun install
bun run setup    # wasm-bindgen-cli at the version Cargo.lock names
```

Then begin a game of your own from `dot`, in a folder beside the console,
and play it:

```sh
bun run new ../mygame
bun run workbench ../mygame
```

- `../mygame` is a cargo workspace of its own that depends on the console's
  crates by path, its lock begun at the versions the console tests. The
  game is yours, under the licence you choose.
- What the console's commands build names its sources from the game's
  folder and the console's: a file you share carries no path of your machine.
- `src/lib.rs` is the game: `cartridge::cart!` exports it, `start` makes
  its state from the seed, `update` reads the buttons and draws.
- `build.rs` gives it its stack and writes `cover.png` into the file as its
  cover; paint your own, 160 by 120 in at most 32 colours.
- `cargo test`, in its folder, runs it built to WebAssembly on wasmi, node
  (V8) and bun (JavaScriptCore), and fails if they disagree on a frame, if
  it stops, or if an update nears the budget.
- No allocator and no clock: fixed arrays, integers and fixed point, and
  chance from the seed.

## The workbench

`bun run workbench [name | folder]` builds every game under `examples/` and
opens a page, on the first free port from 5180, that runs one on the
browser's engine and shows it from inside: its screen, palette, meter and
memory, with its sound through the chip.

- With no name it opens `rocks`; `bun run workbench dot` opens dot.
- A folder opens a game elsewhere, built in its own workspace.
- It watches every game's folder: a saved file rebuilds that game, and the
  page runs the new build through the game played so far.
- A page hidden holds the game where it was last shown, its sound eased
  out; shown again, both run on as if never hidden.

## Headless runs

`bun run cart <name | folder | file.wasm>` builds a game in release, prints
its size and the name its hash gives it, and runs it with no window on
wasmi.

```sh
bun run cart ../mygame --buttons '0:right 40:down+a 41:down 80:a' --frames 120 --every 40
bun run cart rocks --buttons @session.txt --frames 351 --wav out/rocks.wav
```

- Out: a PNG every `--every` updates and `strip.png` with them all in
  `out/<name>/`, the frames' hash, the meter's mean and peak, the tones
  played, and the stop if one came.
- `--buttons`: `UPDATE:NAMES` items, each held until the next; names joined
  by `+` (`left` `right` `up` `down` `a` `b` `start`, the first pad's;
  `2.a` and the like, the second's), `-` for none.
- `--wav FILE`: the tones through the chip as a 48 kHz WAV.
- `--seed N`, `--frames N`, `--out DIR`, `--at N,N`, `--scale N`.

## The emulator

`bun run emulator [folder]` builds the emulator into one folder,
`target/emulator` by default, each file named by the SHA-256 of its bytes
and `emulator.json` naming them: the runner and `host.js`, the chip and
its processor, and the cases every host is held to. The workbench and the
tests run it.

- Its bytes are the platform's that builds it, so a host pins the build of
  one image, Linux on x86-64 by digest: `bun run emulator --pin <folder>`,
  with Docker.
- `--same <folder>` builds it in that image again and names each file that
  differs.
- Built on another platform, it says when its files are not those of the
  pin a host above the console keeps.

## What is here

| Path | What |
|---|---|
| `crates/cartridge` | the contract as types and constants, `Screen`, the font, `cart!` and `cover!`; `no_std` |
| `crates/cartridge-build` | what a game's build script calls: its stack and its cover |
| `crates/cartridge-run` | prepares a file and runs it on any engine behind one trait |
| `crates/cartridge-wasmi` | wasmi, the reference; its `hostile` and `contract` suites are the contract's proof |
| `crates/runner` | the emulator's runner, a host's games in one WebAssembly module, and `host.js`, its host on a JavaScript engine |
| `crates/chip`, `crates/chip-worklet` | the sound chip, and it as an AudioWorklet module |
| `crates/headless` | the runner behind `bun run cart` |
| `web/` | the workbench's page |
| `examples/` | `rocks`, a ship against drifting rocks; `dot`; `boom`, tones that move; `hues`, all 32 colours |

`bun run check` runs fmt, clippy, the tests, the hostile and contract
suites on wasmi and through the emulator on node, bun and SpiderMonkey, the
chip's goldens on V8, and the workbench page's check.

## License

MIT: [LICENSE](LICENSE). Copyright (C) 2026 Tekne Studio.

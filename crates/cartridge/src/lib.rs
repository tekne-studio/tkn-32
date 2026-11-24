//! A cartridge: a small game as one WebAssembly file, and the contract a
//! runner holds it to. This crate is the contract as types and constants, a
//! screen to draw on, and [`cart!`], which exports a [`Game`] by the
//! contract's names, so a game in Rust is a few hundred lines of its own.
//!
//! # The contract, version [`ABI`]
//!
//! A cartridge is one `.wasm` file, named by the hash of its content like
//! any heavy file. It reaches nothing: no clock, no random source, no
//! network, no file, nothing of its host. Any language that builds a
//! WebAssembly module writes one.
//!
//! It exports:
//!
//! | name | type | |
//! |---|---|---|
//! | `cart_abi` | `() -> i32` | the contract's version, [`ABI`] |
//! | `init` | `(seed: i32)` | called once, before any update |
//! | `update` | `(buttons: i32)` | called [`TICKS`] times a second, with the [`button`]s held |
//! | `frame` | `() -> i32` | the address of the screen: [`WIDTH`] by [`HEIGHT`] bytes, row by row from the top left, each an index into the palette (its low five bits) |
//! | `palette` | `() -> i32` | the address of [`COLOURS`] colours, four bytes each: red, green, blue, alpha |
//! | `memory` | memory | its own memory, not imported |
//! | `screen` | `() -> i32` | optional: the step of its screen; [`SCREENS`] are listed |
//!
//! A runner asks for `frame` and `palette` after every update and copies
//! both out, so a cartridge may move them between updates. It asks for
//! `screen` once, after `cart_abi` and before `init`; one that exports none
//! draws step 0, [`WIDTH`] by [`HEIGHT`].
//!
//! It may import one function, all `i32`, no result: `env.tone(channel,
//! hz, ms, wave, to_hz, loud, to_loud)` plays a tone for `ms` milliseconds
//! on one of [`CHANNELS`] channels in one of the [`wave`]s, replacing what
//! that channel plays. Its pitch slides by equal ratios from `hz` to
//! `to_hz` and its loudness goes in a straight line from `loud` to
//! `to_loud`, out of [`LOUDEST`], over its length; `hz` 0, or both
//! loudnesses 0, silences the channel. [`tone`] and [`tone_to`] call it.
//! A runner with no sound ignores it. It imports nothing else.
//!
//! # Limits
//!
//! A runner refuses a file over [`MAX_FILE`] bytes; a memory over
//! [`MAX_MEMORY`]; more than one table, or one over [`MAX_TABLE`]
//! elements; a function whose frame, its parameters, its locals and the
//! deepest its operand stack goes, holds over [`MAX_FRAME`] values; a data
//! or element segment outside its memory or its table; and anything the
//! runner's validator does not accept.
//!
//! It counts the work of each call in units. A call of a function takes its
//! declared locals and the instructions it holds outside the loops inside
//! it, a turn of a loop the instructions the loop holds outside the loops
//! inside it, whichever branch runs; a bulk memory instruction takes one
//! per [`BULK_BYTES`] bytes it moves or fills. It gives the start
//! function, `cart_abi`, `screen` and `init` together [`START_BUDGET`], and
//! each update with its `frame` and `palette` [`UPDATE_BUDGET`].
//!
//! It counts the stack: a call the cartridge makes holds its callee's frame
//! and [`FRAME_OVERHEAD`] more until it returns, and the calls held at once
//! hold at most [`MAX_STACK`]. The engines measured, wasmi, V8 and
//! JavaScriptCore, run deeper than that, so a cartridge stops at the same
//! call on each.
//!
//! A cartridge over any of them, or one that traps, stops, and the screen
//! says so.
//!
//! # Determinism
//!
//! The same seed and the same buttons give the same frames on every runner,
//! so a game is tested by a strip of pictures and a score is replayed. One
//! exception: WebAssembly leaves the bits of a NaN to the engine, so a
//! cartridge whose state depends on them, by storing a NaN's bits or
//! branching on them, may differ between runners. Floats are allowed; a
//! game that must replay exactly keeps NaN out of its state, or counts in
//! integers and fixed point.

#![no_std]

#[doc(hidden)]
pub mod abi;
pub mod cover;
pub mod font;
pub mod pack;
mod screen;

pub use abi::{tone, tone_to};
pub use cover::Cover;
pub use screen::Screen;

/// The contract's version, the one a cartridge says it speaks by
/// `cart_abi`.
pub const ABI: u32 = 1;

/// The screen's width in pixels, at step 0.
pub const WIDTH: usize = 160;
/// The screen's height in pixels, at step 0.
pub const HEIGHT: usize = 120;
/// The screen's bytes, one a pixel.
pub const PIXELS: usize = WIDTH * HEIGHT;
/// The screens a runner knows, by step: step `n` would be `WIDTH << n` by
/// `HEIGHT << n`, and only step 0 is listed. A cartridge that asks for a
/// step past the list stops; a runner that lists more runs it.
pub const SCREENS: u32 = 1;
/// The colours of a palette.
pub const COLOURS: usize = 32;
/// Updates a second.
pub const TICKS: u32 = 60;

/// The largest cartridge file, in bytes.
pub const MAX_FILE: usize = 256 * 1024;
/// The most memory a cartridge has, in bytes: sixteen pages of 64 KiB.
pub const MAX_MEMORY: usize = 1024 * 1024;
/// The units of work the start function, `cart_abi`, `screen` and `init`
/// share.
pub const START_BUDGET: u32 = 20_000_000;
/// The units of work of one update, its `frame` and its `palette`: about
/// eight times the heaviest update of `rocks`. Spent whole by a loop that
/// never ends, it is a few milliseconds of an interpreter.
pub const UPDATE_BUDGET: u32 = 2_000_000;
/// The bytes a bulk memory instruction moves or fills for one unit of work.
pub const BULK_BYTES: u32 = 16;
/// The most elements of a cartridge's one table.
pub const MAX_TABLE: u32 = 4096;
/// The most values a function's frame holds: its parameters, its locals
/// and the deepest its operand stack goes.
pub const MAX_FRAME: u32 = 4096;
/// What a call holds beside its callee's frame, in values: what an engine
/// keeps for the call itself.
pub const FRAME_OVERHEAD: u32 = 8;
/// The most values the calls a cartridge makes hold at once, each its
/// callee's frame and [`FRAME_OVERHEAD`]. V8, the tightest engine measured,
/// spends about 8.3 bytes a value and 64 a call from a stack of 984 KiB:
/// this is about a seventh of it.
pub const MAX_STACK: u32 = 16_384;

/// The channels a cartridge plays tones on.
pub const CHANNELS: u32 = 4;
/// The tones one update may play; a runner drops the rest.
pub const MAX_TONES: usize = 16;
/// The most a tone's loudness is, and a steady [`tone`]'s: a louder one is
/// held to it.
pub const LOUDEST: u32 = 255;

/// Four bytes a colour: red, green, blue, alpha.
pub type Palette = [[u8; 4]; COLOURS];

/// The functions a cartridge exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Export {
    CartAbi,
    Init,
    Update,
    Frame,
    Palette,
    Screen,
}

impl Export {
    /// Those every cartridge exports, in the contract's order.
    pub const REQUIRED: [Export; 5] = [
        Export::CartAbi,
        Export::Init,
        Export::Update,
        Export::Frame,
        Export::Palette,
    ];
    /// Those a cartridge may export; one it lacks gives 0.
    pub const OPTIONAL: [Export; 1] = [Export::Screen];

    /// Its name among the module's exports.
    pub const fn name(self) -> &'static str {
        match self {
            Export::CartAbi => "cart_abi",
            Export::Init => "init",
            Export::Update => "update",
            Export::Frame => "frame",
            Export::Palette => "palette",
            Export::Screen => "screen",
        }
    }

    pub const fn signature(self) -> Signature {
        let (params, results) = match self {
            Export::CartAbi | Export::Frame | Export::Palette | Export::Screen => (0, 1),
            Export::Init | Export::Update => (1, 0),
        };
        Signature { params, results }
    }
}

/// A function's type in the contract: its parameters and its results, each
/// an `i32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signature {
    pub params: usize,
    pub results: usize,
}

/// The name a cartridge exports its memory by.
pub const MEMORY: &str = "memory";
/// The function a cartridge may import, by its module and its name.
pub const TONE: [&str; 2] = ["env", "tone"];
pub const TONE_SIGNATURE: Signature = Signature {
    params: 7,
    results: 0,
};

/// The buttons, one bit each in the number `update` takes.
pub mod button {
    pub const LEFT: u32 = 1 << 0;
    pub const RIGHT: u32 = 1 << 1;
    pub const UP: u32 = 1 << 2;
    pub const DOWN: u32 = 1 << 3;
    pub const A: u32 = 1 << 4;
    pub const B: u32 = 1 << 5;
    pub const START: u32 = 1 << 6;
    /// Every button the contract names; a runner sends no other bit.
    pub const ALL: u32 = (1 << 7) - 1;
}

/// The waves a tone is played in.
pub mod wave {
    pub const SQUARE: u32 = 0;
    pub const TRIANGLE: u32 = 1;
    pub const SAW: u32 = 2;
    pub const NOISE: u32 = 3;
    /// How many there are: a tone in any other is dropped.
    pub const COUNT: u32 = 4;
}

/// The buttons held during one update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons(pub u32);

impl Buttons {
    /// Whether every button of `mask` is held.
    pub fn held(self, mask: u32) -> bool {
        self.0 & mask == mask
    }

    /// The buttons held now that were not held `before`.
    pub fn pressed_since(self, before: Buttons) -> Buttons {
        Buttons(self.0 & !before.0)
    }
}

/// A game, as [`cart!`] exports it.
pub trait Game: Send + Sized {
    /// The game at its start, from the seed the runner gives `init`.
    fn start(seed: u32) -> Self;
    /// One tick: read the buttons, move the world, draw it.
    fn update(&mut self, buttons: Buttons, screen: &mut Screen);
    /// The colours the screen's indices name, read after each update.
    fn palette(&self) -> Palette;
}

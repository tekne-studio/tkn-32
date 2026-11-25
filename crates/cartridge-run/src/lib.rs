//! Runs a cartridge (the contract is `cartridge`'s): its file prepared once
//! ([`prepare`]: the limits, the exports cut to the contract's, a meter of
//! work and a count of the stack written into its code), then instantiated
//! on an engine behind [`Instance`] and held to the contract by [`Cart`],
//! and kept running by a clock on a [`Shelf`]. The emulator's runner runs
//! it on a host's engine, whatever the host; `cartridge-wasmi` runs it on
//! wasmi, the suites' reference.
//!
//! What it refuses, at preparation: a file over `cartridge::MAX_FILE`; one
//! the validator refuses, which takes WebAssembly 2.0 without SIMD (so no
//! threads, tail calls, exceptions, second or 64-bit memory, extended
//! constants, custom page sizes or GC); an import other than `env.tone` of
//! its type, a memory among them; a memory shared or starting over
//! `cartridge::MAX_MEMORY`; more than one table, or one over
//! `cartridge::MAX_TABLE`; a function whose frame holds over
//! `cartridge::MAX_FRAME` values; a data or element segment outside its
//! memory or its table; `table.grow`, `table.fill`, `table.copy` and
//! `table.init`; a contract export missing or of another type. An optional
//! export of another type, and any export the contract does not name, is
//! dropped, so no engine can call it. A start function is the runner's to
//! call, metered, under the start budget.
//!
//! What it stops, at the start ([`Stop`]): another version than
//! `cartridge::ABI`, and a screen off the list.
//!
//! A cartridge's cover ([`cover`]) is read from its file as it came, never
//! by running it: preparing a file drops its custom sections. An idle
//! cabinet shows it as a cart that never runs ([`still`]).

mod cart;
mod colour;
mod cover;
mod frames;
mod keys;
mod prepare;
mod read;
mod shelf;
mod tones;
mod watch;

pub use cart::{Cart, Instance, Stop, Trapped};
pub use cartridge::Cover;
pub use colour::{Resolved, SHOWN_BYTES, draw, resolve, shown};
pub use cover::{cover, cover_rgba, cover_screen, still};
pub use frames::Frames;
pub use keys::{KEYS, SHARED, button};
pub use prepare::{METER, OUT_OF_MEMORY, Prepared, Refusal, START, TOO_DEEP, prepare, shaped};
pub use read::{READ_BYTES, read_bytes};
pub use shelf::{CATCH_UP, FASTEST, KEPT, LONGEST, Shelf};
pub use tones::{Tone, Tones, WORDS, batch};
pub use watch::{BLOCK, Ran, Watch};

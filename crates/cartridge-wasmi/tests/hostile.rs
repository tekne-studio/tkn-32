//! Cartridges written to break the contract, one file a way of attack: the
//! contract's proof. Each case runs on wasmi and, once prepared, on every
//! browser engine at hand, and each stops the same way after the same
//! update on all of them. A new finding lands here as a case.
//!
//! Natively the browser engines are node and bun, as processes. Built to
//! WebAssembly, it is `cartridge-web`'s test too, and the engine is the
//! host's own through `Browser`: each case file says `test` is
//! wasm-bindgen-test's there.
//!
//! - `time`: work past the budget, in every shape the meter prices
//! - `memory`: memory past the ceiling, or written where it is not
//! - `stack`: calls past the stack, frames past a frame
//! - `tables`: the engine's tables past their limits
//! - `imports`: a way out by an import
//! - `exports`: a way out, or in, by an export
//! - `start`: a way past the runner by a start function
//! - `pointers`: a screen or a palette that lies
//! - `malformed`: files that are not cartridges, whole or cut
//! - `cover`: a cover's section that lies, read before anything runs

#[path = "support/mod.rs"]
mod support;

#[path = "hostile/cover.rs"]
mod cover;
#[path = "hostile/exports.rs"]
mod exports;
#[path = "hostile/imports.rs"]
mod imports;
#[path = "hostile/malformed.rs"]
mod malformed;
#[path = "hostile/memory.rs"]
mod memory;
#[path = "hostile/pointers.rs"]
mod pointers;
#[path = "hostile/stack.rs"]
mod stack;
#[path = "hostile/start.rs"]
mod start;
#[path = "hostile/tables.rs"]
mod tables;
#[path = "hostile/time.rs"]
mod time;

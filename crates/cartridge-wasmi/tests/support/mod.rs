//! What the tests of a cartridge's runner share: a cartridge written by
//! hand, and its run on wasmi and through every host of the emulator at
//! hand (`cartridge_test::on_every_engine`). Each test binary uses a part.
#![allow(dead_code, unused_imports)]

pub use cartridge_test::{Run, emulator, fnv, on_every_engine, on_wasmi, part};

use cartridge_run::Stop;

/// A cartridge that keeps the contract, with `update`'s body, `more`
/// fields in the module and `memory` pages, as WebAssembly text. Its screen
/// is at 4096 and its palette at 0; `$seed` holds what `init` was given.
/// `env.tone` is `$tone_to`, and `$tone` plays a steady tone through it,
/// as `cartridge::tone` does.
pub fn cart(update: &str, more: &str, memory: u32) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
            (import "env" "tone" (func $tone_to (param i32 i32 i32 i32 i32 i32 i32)))
            (func $tone (param i32 i32 i32 i32)
                (call $tone_to (local.get 0) (local.get 1) (local.get 2) (local.get 3)
                    (local.get 1) (i32.const 255) (i32.const 255)))
            (memory (export "memory") {memory})
            (global $seed (mut i32) (i32.const 0))
            (func (export "cart_abi") (result i32) (i32.const 1))
            (func (export "init") (param i32) (global.set $seed (local.get 0)))
            (func (export "update") (param $buttons i32) {update})
            (func (export "frame") (result i32) (i32.const 4096))
            (func (export "palette") (result i32) (i32.const 0))
            {more})"#
    ))
    .expect("the test's cartridge is valid text")
}

/// Asserts `file` stops as `stop`, after `after` updates, on every engine.
pub fn assert_stops(file: &[u8], stop: Stop, after: u64) {
    let buttons = vec![0; after as usize + 2];
    let run = on_every_engine(file, 1, &buttons);
    assert_eq!(
        (&run.stop, run.updates),
        (&stop.to_string(), after),
        "{run}"
    );
}

/// Asserts `file` runs `updates` updates on every engine without stopping.
pub fn assert_runs(file: &[u8], updates: u64) {
    let run = on_every_engine(file, 1, &vec![0; updates as usize]);
    assert_eq!(
        (run.stop.as_str(), run.updates),
        ("running", updates),
        "{run}"
    );
}

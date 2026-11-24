//! Lies about the screen and the palette: `frame` and `palette` are the
//! cartridge's word for where they are, asked after every update. Wherever
//! one points, all of it inside the memory or the cartridge stops as
//! `Screen`, never a read outside it, and an answer that takes work is
//! charged to the update.

use cartridge::{COLOURS, PIXELS};
use cartridge_run::Stop;

use crate::support::{assert_runs, assert_stops};

const PAGE: i64 = 65536;
const COLOURS_BYTES: i64 = COLOURS as i64 * 4;

/// A cartridge of one page whose `frame` and `palette` give what the two
/// bodies leave; `$n` counts its updates.
fn pointing(frame: &str, palette: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module (memory (export "memory") 1)
            (global $n (mut i32) (i32.const 0))
            (func (export "cart_abi") (result i32) (i32.const 1))
            (func (export "init") (param i32))
            (func (export "update") (param i32) (global.set $n (i32.add (global.get $n) (i32.const 1))))
            (func (export "frame") (result i32) {frame})
            (func (export "palette") (result i32) {palette}))"#
    ))
    .expect("the test's cartridge is valid text")
}

/// `at` as an i32 constant, its bits whatever the cartridge means by it.
fn at(at: i64) -> String {
    format!("(i32.const {})", at as u32 as i32)
}

#[test]
fn a_screen_to_the_last_byte_runs_and_one_past_stops() {
    let last = PAGE - PIXELS as i64;
    assert_runs(&pointing(&at(last), &at(0)), 2);
    assert_stops(&pointing(&at(last + 1), &at(0)), Stop::Screen, 0);
}

#[test]
fn a_palette_to_the_last_byte_runs_and_one_past_stops() {
    let last = PAGE - COLOURS_BYTES;
    assert_runs(&pointing(&at(4096), &at(last)), 2);
    assert_stops(&pointing(&at(4096), &at(last + 1)), Stop::Screen, 0);
}

#[test]
fn pointers_past_any_memory_stop_it() {
    // Negative as an i32, near the top of the addresses, and where adding
    // a screen's or a palette's length wraps a 32-bit sum back to 0.
    for lie in [
        -1,
        i64::from(i32::MIN),
        u32::MAX as i64 - PIXELS as i64 + 1,
        u32::MAX as i64 - COLOURS_BYTES + 1,
        PAGE,
    ] {
        assert_stops(&pointing(&at(lie), &at(0)), Stop::Screen, 0);
        assert_stops(&pointing(&at(4096), &at(lie)), Stop::Screen, 0);
    }
}

#[test]
fn a_screen_that_moves_out_stops_at_the_update_it_moved() {
    let moves = "(if (result i32) (i32.lt_u (global.get $n) (i32.const 3))
        (then (i32.const 4096)) (else (i32.const 65000)))";
    assert_stops(&pointing(moves, &at(0)), Stop::Screen, 3);
}

#[test]
fn a_screen_that_takes_work_to_find_is_charged_or_stops() {
    let forever = "(loop $again (br $again)) (i32.const 0)";
    assert_stops(&pointing(forever, &at(0)), Stop::Budget, 0);
    assert_stops(&pointing(&at(4096), forever), Stop::Budget, 0);
    assert_stops(&pointing("unreachable", &at(0)), Stop::Trap, 0);
}

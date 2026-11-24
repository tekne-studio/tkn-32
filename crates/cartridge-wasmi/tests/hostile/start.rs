//! A way past the runner by a start function: an engine runs a start
//! function as it instantiates, before the runner holds anything. The
//! prepared file exports it instead, and the runner calls it first, under
//! the start budget it shares with `cart_abi` and `init`, so it stops as
//! anything else would.

use cartridge_run::Stop;

use crate::support::{assert_runs, assert_stops};

/// A cartridge whose start function and `init` run `start` and `init`.
fn starting(start: &str, init: &str) -> Vec<u8> {
    let file = wat::parse_str(format!(
        r#"(module
            (memory (export "memory") 1)
            (global $seed (mut i32) (i32.const 0))
            (func $start {start}) (start $start)
            (func (export "cart_abi") (result i32) (i32.const 1))
            (func (export "init") (param i32) {init})
            (func (export "update") (param i32))
            (func (export "frame") (result i32) (i32.const 4096))
            (func (export "palette") (result i32) (i32.const 0)))"#
    ));
    file.expect("the test's cartridge is valid text")
}

/// `fills` fills of 64 KiB, each 4096 units.
fn fills(fills: u32) -> String {
    format!(
        "(local $n i32) (loop $again
            (memory.fill (i32.const 0) (i32.const 0) (i32.const 65536))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (br_if $again (i32.lt_u (local.get $n) (i32.const {fills}))))"
    )
}

#[test]
fn a_start_function_runs_before_init() {
    // It sets the seed's global, and init finds it set.
    let file = starting(
        "(global.set $seed (i32.const 9))",
        "(if (i32.ne (global.get $seed) (i32.const 9)) (then unreachable))",
    );
    assert_runs(&file, 2);
}

#[test]
fn a_start_function_that_never_ends_is_over_its_budget() {
    assert_stops(&starting("(loop $again (br $again))", ""), Stop::Budget, 0);
}

#[test]
fn a_start_function_stops_as_anything_else_would() {
    assert_stops(&starting("(call $start)", ""), Stop::Stack, 0);
    assert_stops(
        &starting("(drop (memory.grow (i32.const 1000)))", ""),
        Stop::Memory,
        0,
    );
    assert_stops(&starting("unreachable", ""), Stop::Trap, 0);
}

#[test]
fn the_start_budget_is_the_start_functions_and_inits_together() {
    // A thousand fills are twice an update's budget and a fifth of the
    // start's: alone, or in both, they run.
    assert_runs(&starting(&fills(1000), ""), 1);
    assert_runs(&starting(&fills(1000), &fills(1000)), 1);
    // Three thousand in each are over it, though either alone is not.
    assert_runs(&starting(&fills(3000), ""), 1);
    assert_stops(&starting(&fills(3000), &fills(3000)), Stop::Budget, 0);
}

#[test]
fn a_start_function_is_not_called_again() {
    // It traps the second time it runs.
    let once = "(if (global.get $seed) (then unreachable)) (global.set $seed (i32.const 1))";
    assert_runs(&starting(once, ""), 3);
}

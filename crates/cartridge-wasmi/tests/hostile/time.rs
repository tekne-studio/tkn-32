//! Exhaustion of time: work past the update's budget, in each shape the
//! meter has to price. Every one stops as `Budget` in its first update.

use cartridge::{MAX_FRAME, UPDATE_BUDGET};

use crate::support::{assert_runs, assert_stops, cart};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn over_budget(update: &str, more: &str) {
    assert_stops(&cart(update, more, 1), "Budget", 0);
}

#[test]
fn a_loop_without_end() {
    over_budget("(loop $again (br $again))", "");
}

#[test]
fn a_branch_table_into_a_loop() {
    over_budget(
        "(loop $again (block $out (br_table $again $out $again (i32.const 0))))",
        "",
    );
}

#[test]
fn a_branch_table_carrying_a_value_into_a_loop() {
    over_budget(
        "(i32.const 5) (loop $again (param i32) (br_table $again $again (i32.const 0)))",
        "",
    );
}

#[test]
fn a_loop_carrying_its_parameters_back() {
    over_budget(
        "(i32.const 0) (loop $again (param i32) (i32.add (i32.const 1)) (br $again))",
        "",
    );
}

#[test]
fn both_arms_of_an_if_branching_back() {
    over_budget(
        "(loop $again (if (local.get $buttons) (then (br $again)) (else (br $again))))",
        "",
    );
}

#[test]
fn a_call_in_a_loop() {
    over_budget(
        "(loop $again (call $nothing) (br $again))",
        "(func $nothing)",
    );
}

#[test]
fn an_indirect_call_in_a_loop() {
    over_budget(
        "(loop $again (call_indirect (type $empty) (i32.const 0)) (br $again))",
        "(type $empty (func)) (table 1 funcref) (elem (i32.const 0) $nothing) (func $nothing)",
    );
}

#[test]
fn bulk_memory_by_its_bytes() {
    // A thousand fills of 64 KiB: few instructions, 4 million units.
    over_budget(
        "(local $n i32) (loop $again
            (memory.fill (i32.const 0) (i32.const 0) (i32.const 65536))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (br_if $again (i32.lt_u (local.get $n) (i32.const 1000))))",
        "",
    );
    over_budget(
        "(loop $again (memory.copy (i32.const 0) (i32.const 1) (i32.const 65535)) (br $again))",
        "",
    );
}

/// A function of `locals` declared locals, called `calls` times.
fn fat_calls(locals: u32, calls: u32) -> Vec<u8> {
    let update = format!(
        "(local $n i32) (loop $again
            (call $fat)
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (br_if $again (i32.lt_u (local.get $n) (i32.const {calls}))))"
    );
    cart(
        &update,
        &format!("(func $fat (local {}))", " i64".repeat(locals as usize)),
        1,
    )
}

#[test]
fn declared_locals_are_charged_at_each_call() {
    // Each call zeroes its locals: the meter takes one unit a local, so a
    // few instructions that call a fat function are charged its locals.
    let locals = MAX_FRAME - 1;
    let fit = UPDATE_BUDGET / locals;
    assert_runs(&fat_calls(locals, fit * 9 / 10), 2);
    assert_stops(&fat_calls(locals, fit + 1), "Budget", 0);
}

#[test]
fn the_meter_writes_valid_code_where_code_cannot_be_reached() {
    // A bulk instruction and a grow after a branch: the meter's own code
    // around them validates against a stack that is anything.
    assert_runs(
        &cart(
            "(block (br 0) (memory.fill) (drop (memory.grow)) (call $nothing))",
            "(func $nothing)",
            1,
        ),
        2,
    );
}

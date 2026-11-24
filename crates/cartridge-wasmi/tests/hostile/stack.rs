//! Exhaustion of the stack: calls that hold more than `MAX_STACK` stop as
//! `Stack` at the same call on every engine, and a function whose frame is
//! over `MAX_FRAME` is refused. The deepest chains the contract allows run
//! on every engine, so no engine's own limit comes first.

use cartridge::{FRAME_OVERHEAD, MAX_FRAME, MAX_STACK};
use cartridge_run::{Refusal, prepare};

use crate::support::{assert_runs, assert_stops, cart};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// `$down` calls itself `n` times, `n + 1` calls in all, each holding a
/// frame of its parameter, `locals` i64 locals, and the two values its
/// operand stack holds at most.
fn chain(n: u32, locals: u32) -> Vec<u8> {
    let locals = if locals == 0 {
        String::new()
    } else {
        format!("(local {})", " i64".repeat(locals as usize))
    };
    cart(
        &format!("(call $down (i32.const {n}))"),
        &format!(
            "(func $down (param $n i32) {locals}
                (if (local.get $n) (then (call $down (i32.sub (local.get $n) (i32.const 1))))))"
        ),
        1,
    )
}

/// The most calls of a frame of `frame` values the stack holds at once.
fn fit(frame: u32) -> u32 {
    MAX_STACK / (frame + FRAME_OVERHEAD)
}

#[test]
fn recursion_without_end() {
    assert_stops(
        &cart("(call $down)", "(func $down (call $down))", 1),
        "Stack",
        0,
    );
}

#[test]
fn mutual_recursion_without_end() {
    let more = "(func $a (call $b)) (func $b (call $a))";
    assert_stops(&cart("(call $a)", more, 1), "Stack", 0);
}

#[test]
fn recursion_through_the_table_without_end() {
    let more = "(type $empty (func)) (table 1 funcref) (elem (i32.const 0) $down)
        (func $down (call_indirect (type $empty) (i32.const 0)))";
    assert_stops(&cart("(call $down)", more, 1), "Stack", 0);
}

#[test]
fn the_deepest_chain_of_small_frames_runs_and_one_more_call_stops() {
    let calls = fit(3);
    assert!(calls > 1000, "{calls} calls: deeper than wasmi's default");
    assert_runs(&chain(calls - 1, 0), 2);
    assert_stops(&chain(calls, 0), "Stack", 0);
}

#[test]
fn the_deepest_chain_of_the_largest_frames_runs_and_one_more_call_stops() {
    let locals = MAX_FRAME - 3;
    let calls = fit(MAX_FRAME);
    assert_runs(&chain(calls - 1, locals), 2);
    assert_stops(&chain(calls, locals), "Stack", 0);
}

#[test]
fn a_frame_over_the_limit_is_refused() {
    // Its locals, or its operand stack.
    let locals = MAX_FRAME - 2;
    assert_eq!(
        prepare(&chain(1, locals)).unwrap_err(),
        Refusal::Frame(MAX_FRAME + 1)
    );
    let pushes = MAX_FRAME as usize;
    let tall = format!(
        "{} {}",
        "(i32.const 1)".repeat(pushes),
        "(drop)".repeat(pushes)
    );
    assert_eq!(
        prepare(&cart(&tall, "", 1)).unwrap_err(),
        Refusal::Frame(MAX_FRAME + 1)
    );
}

#[test]
fn locals_past_what_any_engine_takes_are_refused() {
    for locals in [30_000, 50_000, 100_000] {
        let file = cart("", &format!("(func (local {}))", " i32".repeat(locals)), 1);
        assert!(prepare(&file).is_err(), "{locals} locals");
    }
}

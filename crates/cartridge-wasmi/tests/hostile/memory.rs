//! Exhaustion of memory, and memory written where it is not: past the
//! ceiling it stops as `Memory`, outside its bounds as `Trap`, and a
//! segment that could not be written is refused before any engine sees it.

use cartridge::MAX_MEMORY;
use cartridge_run::{Refusal, Stop, prepare};

use crate::support::{assert_runs, assert_stops, cart};

const PAGES: u32 = (MAX_MEMORY / 65536) as u32;

#[test]
fn growing_past_the_ceiling_in_a_loop() {
    let grows = "(loop $again (drop (memory.grow (i32.const 1))) (br $again))";
    assert_stops(&cart(grows, "", 1), Stop::Memory, 0);
}

#[test]
fn growing_past_the_ceiling_at_once() {
    assert_stops(
        &cart("(drop (memory.grow (i32.const 1000)))", "", 1),
        Stop::Memory,
        0,
    );
}

#[test]
fn growing_to_the_ceiling_runs() {
    let to_the_ceiling = format!(
        "(if (i32.eq (memory.grow (i32.const {})) (i32.const -1)) (then unreachable))",
        PAGES - 1
    );
    assert_runs(&cart(&to_the_ceiling, "", 1), 1);
}

#[test]
fn a_maximum_declared_past_the_ceiling_is_the_ceiling() {
    let file = wat::parse_str(format!(
        r#"(module (memory (export "memory") 1 {})
            (func (export "cart_abi") (result i32) (i32.const 1))
            (func (export "init") (param i32))
            (func (export "update") (param i32) (drop (memory.grow (i32.const {PAGES}))))
            (func (export "frame") (result i32) (i32.const 4096))
            (func (export "palette") (result i32) (i32.const 0)))"#,
        PAGES * 4
    ))
    .unwrap();
    assert_stops(&file, Stop::Memory, 0);
}

#[test]
fn a_memory_that_starts_past_the_ceiling_is_refused() {
    assert_eq!(
        prepare(&cart("", "", PAGES + 1)).unwrap_err(),
        Refusal::Memory
    );
}

#[test]
fn a_store_past_the_end_traps() {
    let store = "(i32.store (i32.const 65534) (i32.const 1))";
    assert_stops(&cart(store, "", 1), Stop::Trap, 0);
}

#[test]
fn a_copy_from_a_segment_past_its_end_traps() {
    let init = "(memory.init $note (i32.const 0) (i32.const 2) (i32.const 8))";
    assert_stops(&cart(init, r#"(data $note "abc")"#, 1), Stop::Trap, 0);
}

#[test]
fn a_segment_outside_the_memory_is_refused() {
    for data in [
        r#"(data (i32.const 65535) "ab")"#,
        r#"(data (i32.const -1) "a")"#,
        r#"(data (i32.const 65537) "")"#,
    ] {
        assert_eq!(
            prepare(&cart("", data, 1)).unwrap_err(),
            Refusal::Segment,
            "{data}"
        );
    }
    // To the last byte, it fits.
    assert_runs(&cart("", r#"(data (i32.const 65534) "ab")"#, 1), 1);
}

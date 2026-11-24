//! Exhaustion of the engine's tables: a table is memory the engine keeps
//! outside the cartridge's, so a cartridge has one, of at most
//! `MAX_TABLE` elements, that never grows; a call through it to nothing,
//! or to the wrong type, traps.

use cartridge::MAX_TABLE;
use cartridge_run::{Refusal, Stop};

use crate::support::{assert_runs, assert_stops, cart, refused};

#[test]
fn more_than_one_table_is_refused() {
    let file = cart("", "(table 1 funcref) (table 1 funcref)", 1);
    assert_eq!(refused(&file), Refusal::Table);
    // A hundred of the largest a table may be: one is the most.
    let many = "(table 4096 funcref)".repeat(100);
    assert_eq!(refused(&cart("", &many, 1)), Refusal::Table);
}

#[test]
fn a_table_past_its_limit_is_refused_and_one_at_it_runs() {
    let over = cart("", &format!("(table {} funcref)", MAX_TABLE + 1), 1);
    assert_eq!(refused(&over), Refusal::Table);
    assert_runs(&cart("", &format!("(table {MAX_TABLE} funcref)"), 1), 1);
}

#[test]
fn instructions_that_grow_fill_or_copy_a_table_are_refused() {
    let table = "(table $t 2 funcref) (elem $e func $nothing) (func $nothing)";
    for (update, name) in [
        (
            "(drop (table.grow $t (ref.null func) (i32.const 1)))",
            "table.grow",
        ),
        (
            "(table.fill $t (i32.const 0) (ref.null func) (i32.const 2))",
            "table.fill",
        ),
        (
            "(table.copy $t $t (i32.const 0) (i32.const 1) (i32.const 1))",
            "table.copy",
        ),
        (
            "(table.init $t $e (i32.const 0) (i32.const 0) (i32.const 1))",
            "table.init",
        ),
    ] {
        assert_eq!(refused(&cart(update, table, 1)), Refusal::Instruction(name));
    }
}

#[test]
fn a_segment_outside_the_table_is_refused() {
    for elements in [
        "(table 1 funcref) (elem (i32.const 1) $nothing) (func $nothing)",
        "(table 1 funcref) (elem (i32.const -1) $nothing) (func $nothing)",
        "(table 2 funcref) (elem (i32.const 0) $nothing $nothing $nothing) (func $nothing)",
    ] {
        assert_eq!(
            refused(&cart("", elements, 1)),
            Refusal::Segment,
            "{elements}"
        );
    }
}

#[test]
fn a_call_through_the_table_to_nothing_or_the_wrong_type_traps() {
    let table = "(type $empty (func)) (table 2 funcref) (elem (i32.const 0) $takes)
        (func $takes (param i32))";
    for update in [
        "(call_indirect (type $empty) (i32.const 1))",
        "(call_indirect (type $empty) (i32.const 0))",
        "(call_indirect (type $empty) (i32.const 2))",
    ] {
        assert_stops(&cart(update, table, 1), Stop::Trap, 0);
    }
}

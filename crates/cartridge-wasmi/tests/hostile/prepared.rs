//! A way past preparing by bytes a host keeps as prepared: a host that
//! keeps what preparing gave opens it again without preparing, so the
//! runner opens only bytes of the shape preparing gives, and anything else
//! stops as a load that failed, before any engine sees it.

use cartridge::MEMORY;
use cartridge_run::{METER, Refusal, prepare, shaped};

use crate::support::cart;

/// A module of `fields`, its memory exported unless `fields` says otherwise.
fn module(fields: &str) -> Vec<u8> {
    wat::parse_str(format!("(module {fields})")).expect("the test's module is valid text")
}

const MEMORY_EXPORTED: &str = r#"(memory (export "memory") 1)"#;
const METERED: &str = r#"(global (export "meter") (mut i32) (i32.const 0))"#;

#[test]
fn every_file_preparing_keeps_is_shaped() {
    let file = cart(
        "(call $tone (i32.const 0) (i32.const 440) (i32.const 50) (i32.const 0))",
        "",
        1,
    );
    let prepared = prepare(&file).expect("the test's cartridge keeps the contract");
    assert_eq!(shaped(prepared.bytes()), Ok(()));
    // A start function becomes an export the runner calls.
    let starting = module(&format!(
        r#"{MEMORY_EXPORTED} (func $start) (start $start)
        (func (export "cart_abi") (result i32) (i32.const 1)) (func (export "init") (param i32))
        (func (export "update") (param i32))
        (func (export "frame") (result i32) (i32.const 4096))
        (func (export "palette") (result i32) (i32.const 0))"#
    ));
    let prepared = prepare(&starting).expect("a start function keeps the contract");
    assert_eq!(shaped(prepared.bytes()), Ok(()));
}

#[test]
fn bytes_of_another_shape_are_refused() {
    let start = "(func $start (loop $ever (br $ever))) (start $start)";
    let refused = [
        (
            module(&format!("{MEMORY_EXPORTED} {METERED} {start}")),
            Refusal::Invalid("a start section".to_owned()),
        ),
        (module(MEMORY_EXPORTED), Refusal::Export(METER)),
        (
            module(&format!(
                r#"{MEMORY_EXPORTED} (global (export "meter") i32 (i32.const 0))"#
            )),
            Refusal::Export(METER),
        ),
        (
            module(&format!(
                r#"{MEMORY_EXPORTED} (global (export "meter") (mut i64) (i64.const 0))"#
            )),
            Refusal::Export(METER),
        ),
        (
            module(&format!(r#"{MEMORY_EXPORTED} (func (export "meter"))"#)),
            Refusal::Export(METER),
        ),
        (module(METERED), Refusal::Export(MEMORY)),
        (
            module(&format!(
                r#"(import "env" "more" (func)) {MEMORY_EXPORTED} {METERED}"#
            )),
            Refusal::Import("env.more".to_owned()),
        ),
        (
            module(&format!(
                r#"(import "env" "tone" (global i32)) {MEMORY_EXPORTED} {METERED}"#
            )),
            Refusal::Import("env.tone".to_owned()),
        ),
    ];
    for (bytes, refusal) in refused {
        assert_eq!(shaped(&bytes), Err(refusal));
    }
    assert!(matches!(shaped(b"not a module"), Err(Refusal::Invalid(_))));
}

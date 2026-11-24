//! A way out, or in, by an export: the prepared file exports the
//! contract's names of the contract's types, its memory and the runner's
//! meter, and nothing of the cartridge's own besides. A name the runner
//! uses, exported by the cartridge, is dropped for the runner's.

use cartridge_run::{Refusal, prepare};

use crate::support::{assert_runs, assert_stops, cart};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The names a prepared file exports.
fn exported(file: &[u8]) -> Vec<String> {
    let prepared = prepare(file).unwrap();
    wasmparser::Parser::new(0)
        .parse_all(prepared.bytes())
        .filter_map(|payload| match payload.unwrap() {
            wasmparser::Payload::ExportSection(reader) => Some(
                reader
                    .into_iter()
                    .map(|export| export.unwrap().name.to_owned())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

const CONTRACT: [&str; 7] = [
    "cart_abi", "init", "update", "frame", "palette", "memory", "meter",
];

#[test]
fn exports_outside_the_contract_are_dropped() {
    let more = r#"(func (export "secret") (result i32) (i32.const 42))
        (global (export "seed") (mut i32) (i32.const 0))
        (table (export "table") 1 funcref)
        (export "also_memory" (memory 0))
        (export "also_update" (func 1))"#;
    assert_eq!(exported(&cart("", more, 1)), CONTRACT);
}

#[test]
fn a_meter_of_its_own_is_dropped_for_the_runners() {
    // Its own "meter" a constant that never runs out: the runner's still
    // counts, and still stops it.
    let more = r#"(global (export "meter") i32 (i32.const 2147483647))"#;
    let file = cart("(loop $again (br $again))", more, 1);
    assert_eq!(exported(&file), CONTRACT);
    assert_stops(&file, "Budget", 0);
}

#[test]
fn a_start_of_its_own_is_dropped_and_never_called() {
    let more = r#"(func (export "start") unreachable)"#;
    let file = cart("", more, 1);
    assert_eq!(exported(&file), CONTRACT);
    assert_runs(&file, 2);
}

/// A module that exports `exports` and nothing else.
fn only(exports: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module (memory 1) (global i32 (i32.const 0))
            (func $abi (result i32) (i32.const 1))
            (func $takes (param i32))
            (func $gives (result i32) (i32.const 4096))
            (func $nothing)
            {exports})"#
    ))
    .expect("the test's module is valid text")
}

#[test]
fn a_missing_export_is_refused() {
    let full = [
        r#"(export "cart_abi" (func $abi))"#,
        r#"(export "init" (func $takes))"#,
        r#"(export "update" (func $takes))"#,
        r#"(export "frame" (func $gives))"#,
        r#"(export "palette" (func $gives))"#,
        r#"(export "memory" (memory 0))"#,
    ];
    let names = ["cart_abi", "init", "update", "frame", "palette", "memory"];
    for (missing, name) in names.iter().enumerate() {
        let mut exports = full.to_vec();
        exports.remove(missing);
        assert_eq!(
            prepare(&only(&exports.concat())).unwrap_err(),
            Refusal::Export(name)
        );
    }
    prepare(&only(&full.concat())).expect("the whole contract");
}

#[test]
fn an_export_of_another_type_is_refused() {
    let rest = r#"(export "init" (func $takes)) (export "update" (func $takes))
        (export "frame" (func $gives)) (export "palette" (func $gives))
        (export "memory" (memory 0))"#;
    for (cart_abi, name) in [
        (r#"(export "cart_abi" (func $nothing))"#, "cart_abi"),
        (r#"(export "cart_abi" (func $takes))"#, "cart_abi"),
        (r#"(export "cart_abi" (global 0))"#, "cart_abi"),
        (r#"(export "cart_abi" (memory 0))"#, "cart_abi"),
    ] {
        assert_eq!(
            prepare(&only(&format!("{cart_abi} {rest}"))).unwrap_err(),
            Refusal::Export(name),
            "{cart_abi}"
        );
    }
    let memory_as_a_function = r#"(export "cart_abi" (func $abi)) (export "init" (func $takes))
        (export "update" (func $takes)) (export "frame" (func $gives))
        (export "palette" (func $gives)) (export "memory" (func $nothing))"#;
    assert_eq!(
        prepare(&only(memory_as_a_function)).unwrap_err(),
        Refusal::Export("memory")
    );
}

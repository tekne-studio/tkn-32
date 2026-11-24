//! A way out by an import: a cartridge imports `env.tone` of the
//! contract's type and nothing else, no memory, no table, no global, so it
//! reaches nothing of the runner but a sound.

use cartridge_run::Refusal;

use crate::support::{assert_runs, refused};

/// A module that keeps the contract but for its `import`.
fn importing(import: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module {import}
            (memory (export "memory") 1)
            (func (export "cart_abi") (result i32) (i32.const 1))
            (func (export "init") (param i32))
            (func (export "update") (param i32))
            (func (export "frame") (result i32) (i32.const 4096))
            (func (export "palette") (result i32) (i32.const 0)))"#
    ))
    .expect("the test's cartridge is valid text")
}

#[test]
fn an_import_other_than_tone_is_refused() {
    for (import, name) in [
        (r#"(import "env" "clock" (func (result i64)))"#, "env.clock"),
        (
            r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#,
            "wasi_snapshot_preview1.fd_write",
        ),
        (r#"(import "env" "table" (table 1 funcref))"#, "env.table"),
        (r#"(import "env" "global" (global i32))"#, "env.global"),
        (
            r#"(import "other" "tone" (func (param i32 i32 i32 i32 i32 i32 i32)))"#,
            "other.tone",
        ),
        (
            r#"(import "env" "sound" (func (param i32 i32 i32 i32 i32 i32 i32)))"#,
            "env.sound",
        ),
    ] {
        assert_eq!(refused(&importing(import)), Refusal::Import(name.into()));
    }
}

#[test]
fn an_imported_memory_is_refused() {
    let file = wat::parse_str(r#"(module (import "env" "memory" (memory 1)))"#).unwrap();
    assert_eq!(refused(&file), Refusal::Import("env.memory".into()));
}

#[test]
fn tone_of_another_type_is_refused() {
    for tone in [
        "(func (param i32))",
        "(func (param i32 i32 i32 i32))",
        "(func (param i32 i32 i32 i32 i32 i32 i32) (result i32))",
        "(func (param i32 i32 i32 i32 i32 i32 i64))",
    ] {
        let import = format!(r#"(import "env" "tone" {tone})"#);
        assert_eq!(
            refused(&importing(&import)),
            Refusal::Import("env.tone".into()),
            "{tone}"
        );
    }
}

#[test]
fn tone_imported_twice_is_still_tone() {
    let tone = r#"(import "env" "tone" (func (param i32 i32 i32 i32 i32 i32 i32)))"#;
    assert_runs(&importing(&format!("{tone} {tone}")), 1);
}

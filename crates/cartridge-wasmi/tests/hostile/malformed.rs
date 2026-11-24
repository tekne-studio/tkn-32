//! Malformed and truncated modules: files that are not cartridges, or a
//! cartridge cut or with a byte changed. Preparing one gives a refusal,
//! never a panic, and whatever preparation lets through, an engine loads.

use std::panic::{AssertUnwindSafe, catch_unwind};

use cartridge::MAX_FILE;
use cartridge_run::{Refusal, Stop, prepare};

use crate::support::{cart, on_every_engine, on_wasmi};

/// A cartridge with each part of a module preparation rewrites: imports,
/// globals, a table and its elements, a start function, loops, calls of
/// both kinds, bulk memory, a grow and data.
fn whole() -> Vec<u8> {
    cart(
        "(local $n i32) (loop $again
            (call $tone (i32.const 0) (i32.const 440) (i32.const 10) (i32.const 0))
            (call_indirect (type $empty) (i32.const 0))
            (memory.fill (i32.const 0) (i32.const 0) (i32.const 64))
            (drop (memory.grow (i32.const 0)))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (br_if $again (i32.lt_u (local.get $n) (i32.const 3))))",
        r#"(type $empty (func)) (table 1 funcref) (elem (i32.const 0) $nothing)
            (func $nothing) (func $start (global.set $seed (i32.const 3))) (start $start)
            (data (i32.const 16) "note")"#,
        1,
    )
}

/// Prepares `file` and, if it is let through, runs it on wasmi: a panic in
/// either, or an engine that will not load what was let through, fails.
/// Gives whether it was let through.
fn holds(file: &[u8], what: &dyn Fn() -> String) -> bool {
    let Ok(prepared) = catch_unwind(AssertUnwindSafe(|| prepare(file))) else {
        panic!("prepare panicked on {}", what());
    };
    if prepared.is_err() {
        return false;
    }
    let Ok(run) = catch_unwind(AssertUnwindSafe(|| on_wasmi(file, 1, &[0, 1]))) else {
        panic!("the runner panicked on {}", what());
    };
    // A load's stop says the engine's words after its own.
    let load = Stop::Load(String::new()).to_string();
    assert!(!run.stop.starts_with(&load), "{}: {run}", what());
    true
}

/// Where the header and each section of `file` end.
fn section_ends(file: &[u8]) -> Vec<usize> {
    let mut ends = vec![8];
    while let Some(&end) = ends.last().filter(|&&end| end < file.len()) {
        let (mut size, mut shift, mut at) = (0, 0, end + 1);
        loop {
            let byte = file[at];
            size |= usize::from(byte & 0x7f) << shift;
            (shift, at) = (shift + 7, at + 1);
            if byte & 0x80 == 0 {
                break;
            }
        }
        ends.push(at + size);
    }
    ends
}

#[test]
fn the_whole_cartridge_runs() {
    assert!(holds(&whole(), &|| "the whole cartridge".into()));
}

#[test]
fn a_cartridge_cut_inside_a_section_is_refused() {
    let file = whole();
    let ends = section_ends(&file);
    assert_eq!(ends.last(), Some(&file.len()));
    for cut in 0..file.len() {
        let let_through = holds(&file[..cut], &|| format!("its first {cut} bytes"));
        assert!(
            !let_through || ends.contains(&cut),
            "cut inside a section at {cut}, let through"
        );
    }
}

#[test]
fn a_cartridge_with_any_byte_changed_is_refused_or_runs() {
    let file = whole();
    let mut let_through = 0;
    for at in 0..file.len() {
        let was = file[at];
        for byte in [
            0,
            1,
            0x40,
            0x7f,
            0x80,
            0xff,
            was ^ 1,
            was ^ 0x40,
            was.wrapping_add(1),
        ] {
            if byte == was {
                continue;
            }
            let mut changed = file.clone();
            changed[at] = byte;
            let what = || format!("byte {at} changed from {was:#04x} to {byte:#04x}");
            if holds(&changed, &what) {
                // Some hundreds are let through: one in 64 runs through
                // the emulator's hosts too, which must agree with wasmi.
                if let_through % 64 == 0 {
                    on_every_engine(&changed, 1, &[0, 1]);
                }
                let_through += 1;
            }
        }
    }
    // Most changes break it; some are a cartridge still, and run.
    assert!(
        let_through > 0,
        "no change let through: the test tries too little"
    );
}

#[test]
fn what_is_not_webassembly_is_refused() {
    for file in [
        &b""[..],
        b"\0asm",
        b"\0asm\x02\0\0\0",
        b"\0asm\x0d\0\x01\0",
        b"(module)",
        b"\x7fELF\x02\x01\x01\0",
    ] {
        assert!(
            matches!(prepare(file), Err(Refusal::Invalid(_))),
            "{file:?}"
        );
    }
}

#[test]
fn a_file_over_its_size_is_refused() {
    let big = format!(r#"(@custom "pad" "{}")"#, "x".repeat(MAX_FILE));
    let file = cart("", &big, 1);
    assert!(file.len() > MAX_FILE);
    assert_eq!(prepare(&file).unwrap_err(), Refusal::TooLarge(file.len()));
}

#[test]
fn features_outside_the_set_are_refused() {
    for (update, more, feature) in [
        ("(drop (v128.const i32x4 0 0 0 0))", "", "simd"),
        ("(drop (i32.atomic.load (i32.const 0)))", "", "threads"),
        ("(return_call $nothing)", "(func $nothing)", "tail calls"),
        ("", "(memory 1)", "a second memory"),
        ("", "(tag $oops) (func (throw $oops))", "exceptions"),
        ("", "(type (struct))", "garbage collection"),
        (
            "",
            "(global i32 (i32.add (i32.const 1) (i32.const 2)))",
            "extended constants",
        ),
    ] {
        let file = cart(update, more, 1);
        assert!(
            matches!(prepare(&file), Err(Refusal::Invalid(_))),
            "{feature}"
        );
    }
    for (module, feature) in [
        ("(module (memory 1 1 shared))", "a shared memory"),
        ("(module (memory i64 1))", "a 64-bit memory"),
    ] {
        let file = wat::parse_str(module).expect("the test's module is valid text");
        assert!(
            matches!(prepare(&file), Err(Refusal::Invalid(_))),
            "{feature}"
        );
    }
}

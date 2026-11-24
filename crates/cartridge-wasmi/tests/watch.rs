//! A cartridge seen from inside, as the workbench sees it: its memory, the
//! bytes each run of updates changed, and the frames it showed, which are
//! the frames it shows run alone.

use cartridge_run::{BLOCK, Cart, Frames, Stop, prepare};
use cartridge_wasmi::Wasmi;

/// A memory page.
const PAGE: usize = 64 * 1024;

/// A cart that counts its updates in the word at 1024 and draws nothing.
const COUNTER: &str = r#"(module
    (memory (export "memory") 1 2)
    (func (export "cart_abi") (result i32) i32.const 1)
    (func (export "init") (param i32))
    (func (export "update") (param i32)
        i32.const 1024
        i32.const 1024
        i32.load
        i32.const 1
        i32.add
        i32.store)
    (func (export "frame") (result i32) i32.const 4096)
    (func (export "palette") (result i32) i32.const 0))"#;

fn watched(file: &[u8]) -> Cart<Wasmi> {
    let loaded = prepare(file)
        .map_err(Stop::Refused)
        .and_then(|prepared| Wasmi::load(&prepared));
    match loaded {
        Ok(instance) => Cart::watched(instance, 1),
        Err(stop) => Cart::stopped(stop),
    }
}

#[test]
fn a_look_marks_the_bytes_the_updates_changed_and_no_other() {
    let mut cart = watched(&wat::parse_str(COUNTER).unwrap());
    cart.update(0);
    cart.update(0);
    cart.look();
    let watch = cart.watch().expect("watched");
    assert_eq!(watch.memory().len(), PAGE);
    assert_eq!(watch.memory()[1024], 2);
    assert!(watch.changed(1024));
    assert!(!watch.changed(1025) && !watch.changed(0));
    let blocks = watch.blocks();
    assert_eq!(blocks.len(), PAGE / BLOCK);
    assert_eq!(blocks.iter().filter(|&&b| b == 1).count(), 1);
    assert_eq!(blocks[1024 / BLOCK], 1);
}

#[test]
fn a_cart_watched_shows_the_frames_it_shows_run_alone() {
    let file = include_bytes!("../../headless/tests/kept/dot.wasm");
    let mut seen = watched(file);
    let mut cart = Wasmi::open(file, 1);
    let mut alone = Frames::default();
    for _ in 0..120 {
        seen.update(0);
        cart.update(0);
        alone.add(&cart);
    }
    assert_eq!(seen.watch().expect("watched").frames(), alone);
    assert_ne!(alone, Frames::default());
}

#[test]
fn a_cart_stopped_before_its_first_update_shows_no_frame() {
    let wrong = COUNTER.replace("(result i32) i32.const 1)", "(result i32) i32.const 0)");
    let mut cart = watched(&wat::parse_str(wrong).unwrap());
    cart.update(0);
    cart.update(0);
    assert_eq!(cart.stop(), Some(&Stop::Abi(0)));
    assert_eq!(cart.watch().expect("watched").frames(), Frames::default());
}

#[test]
fn a_stopped_cart_keeps_the_memory_it_stopped_with() {
    let trap = COUNTER.replace("i32.store)", "i32.store unreachable)");
    let mut cart = watched(&wat::parse_str(trap).unwrap());
    cart.update(0);
    assert_eq!(cart.stop(), Some(&Stop::Trap));
    let watch = cart.watch().expect("watched");
    assert_eq!(watch.memory()[1024], 1, "the store before the trap");
    // Nothing runs after the stop: the memory and the marks of the update
    // that stopped it stay as they were.
    cart.update(0);
    cart.look();
    let watch = cart.watch().expect("watched");
    assert_eq!(watch.memory()[1024], 1);
    assert!(watch.changed(1024));
}

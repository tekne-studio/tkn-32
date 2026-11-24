//! What a cartridge that keeps the contract is given: its seed and its
//! buttons, its tones, the same frames from the same input on every
//! engine, and a screen that says why when it stops. What breaks the
//! contract is in `hostile`.

#[path = "support/mod.rs"]
mod support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use cartridge::wave;
use cartridge_run::{Cart, Stop, Tone};
use cartridge_wasmi::Wasmi;
use support::{assert_runs, assert_stops, cart, on_every_engine};

fn run(file: &[u8], updates: u32) -> Cart<Wasmi> {
    let mut cart = Wasmi::open(file, 1);
    for _ in 0..updates {
        cart.update(0);
    }
    cart
}

#[test]
fn a_cartridge_that_keeps_the_contract_runs() {
    // Paints its seed and its buttons, so both are seen to arrive.
    let update = "(i32.store8 (i32.const 4096) (global.get $seed)) (i32.store8 (i32.const 4097) (local.get $buttons))";
    let mut cart = Wasmi::open(&cart(update, "", 1), 7);
    cart.update(0b101);
    assert_eq!(cart.stop(), None);
    assert_eq!(cart.screen()[..2], [7, 0b101]);
    assert_eq!(cart.updates(), 1);
}

#[test]
fn a_trap_stops_it_and_the_screen_says_so() {
    let cart = run(&cart("unreachable", "", 1), 3);
    assert_eq!(cart.stop(), Some(&Stop::Trap));
    // The stop screen is drawn in its own colours: never all background.
    assert!(cart.screen().iter().any(|&pixel| pixel != 0));
    assert_ne!(cart.palette()[0], cart.palette()[1]);
}

/// A cartridge saying `abi`, with `more` in the module.
fn saying(abi: u32, more: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module (memory (export "memory") 1)
            (func (export "cart_abi") (result i32) (i32.const {abi}))
            (func (export "init") (param i32))
            (func (export "update") (param i32))
            (func (export "frame") (result i32) (i32.const 4096))
            (func (export "palette") (result i32) (i32.const 0))
            {more})"#,
    ))
    .unwrap()
}

#[test]
fn another_version_of_the_contract_is_stopped() {
    assert_runs(&saying(1, ""), 2);
    assert_stops(&saying(0, ""), "Abi(0)", 0);
    assert_stops(&saying(2, ""), "Abi(2)", 0);
    assert_eq!(
        Stop::Abi(2).to_string(),
        "it speaks contract 2, this runner 1"
    );
}

#[test]
fn it_draws_the_screen_it_asks_for_from_the_list() {
    let screen = |step: &str| format!(r#"(func (export "screen") (result i32) {step})"#);
    assert_runs(&saying(1, &screen("(i32.const 0)")), 2);
    assert_stops(&saying(1, &screen("(i32.const 1)")), "Size(1)", 0);
    assert_stops(&saying(1, &screen("(i32.const -1)")), "Size(4294967295)", 0);
    assert_stops(&saying(1, &screen("unreachable")), "Trap", 0);
    // One that spends the start budget stops before `init`.
    let forever = screen("(loop $again (br $again)) (i32.const 0)");
    assert_stops(&saying(1, &forever), "Budget", 0);
    // One of another type is dropped, as a name outside the contract is:
    // step 0.
    let other = r#"(func (export "screen") (param i32) (result i32) (i32.const 1))"#;
    assert_runs(&saying(1, other), 2);
    assert_eq!(
        Stop::Size(1).to_string(),
        "it asks for screen 1, this runner knows 0 to 0"
    );
}

#[test]
fn tones_are_heard_up_to_the_most_an_update_keeps() {
    // One on a channel the contract lacks, then steady and moving tones in
    // turn, forty of them: the first sixteen the contract allows are kept.
    let plays = "(local $n i32)
        (call $tone (i32.const 9) (i32.const 440) (i32.const 100) (i32.const 0))
        (loop $again
            (call $tone (i32.const 1) (i32.const 440) (i32.const 100) (i32.const 1))
            (call $tone_to (i32.const 2) (i32.const 160) (i32.const 250) (i32.const 1)
                (i32.const 45) (i32.const 255) (i32.const 0))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (br_if $again (i32.lt_u (local.get $n) (i32.const 20))))";
    let file = cart(plays, "", 1);
    let tones = Wasmi::open(&file, 1).update(0).to_vec();
    let steady = Tone::steady(1, 440, 100, wave::TRIANGLE as u8);
    let boom = Tone::new(2, 160, 250, wave::TRIANGLE as u8, 45, 255, 0);
    assert_eq!(tones, [steady, boom].repeat(cartridge::MAX_TONES / 2));
    let run = on_every_engine(&file, 1, &[0; 3]);
    assert_eq!((run.stop.as_str(), run.updates), ("running", 3));
}

#[test]
fn a_tone_moves_and_every_engine_hears_the_same() {
    // A boom on each update, its loudness from the buttons, a steady tone,
    // a loudness held to the loudest, and a channel out of bounds dropped.
    let plays = "(call $tone_to (i32.const 0) (i32.const 160) (i32.const 250) (i32.const 1)
            (i32.const 45) (i32.add (i32.const 200) (local.get $buttons)) (i32.const 0))
        (call $tone (i32.const 2) (i32.const 880) (i32.const 60) (i32.const 0))
        (call $tone_to (i32.const 3) (i32.const 6000) (i32.const 400) (i32.const 3)
            (i32.const 800) (i32.const 9000) (i32.const 40))
        (call $tone_to (i32.const 4) (i32.const 1) (i32.const 1) (i32.const 0)
            (i32.const 1) (i32.const 1) (i32.const 1))";
    let file = cart(plays, "", 1);
    let mut wasmi = Wasmi::open(&file, 1);
    assert_eq!(
        wasmi.update(7),
        [
            Tone::new(0, 160, 250, wave::TRIANGLE as u8, 45, 207, 0),
            Tone::steady(2, 880, 60, wave::SQUARE as u8),
            Tone::new(3, 6000, 400, wave::NOISE as u8, 800, 255, 40),
        ]
    );
    let buttons: Vec<u32> = (0..20).map(|tick| tick % 8).collect();
    let run = on_every_engine(&file, 1, &buttons);
    assert_eq!((run.stop.as_str(), run.updates), ("running", 20));
    let quiet = on_every_engine(&cart("", "", 1), 1, &buttons);
    assert_ne!(run.tones, quiet.tones, "the tones are in the run");
}

#[test]
fn the_same_seed_and_buttons_give_the_same_frames_on_every_engine() {
    // A cartridge that draws noise from its seed and its buttons.
    let noise = "(local $i i32) (loop $again
        (global.set $seed (i32.add (i32.mul (global.get $seed) (i32.const 1103515245)) (i32.add (local.get $buttons) (i32.const 12345))))
        (i32.store8 (i32.add (i32.const 4096) (local.get $i)) (i32.shr_u (global.get $seed) (i32.const 24)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br_if $again (i32.lt_u (local.get $i) (i32.const 19200))))";
    let file = cart(noise, "", 1);
    let buttons: Vec<u32> = (0..30).map(|tick| tick % 5).collect();
    let run = |seed| on_every_engine(&file, seed, &buttons);
    let three = run(3);
    assert_eq!((three.stop.as_str(), three.updates), ("running", 30));
    assert_eq!(three, run(3));
    assert_ne!(three.hash, run(4).hash);
}

#[test]
fn a_kept_file_runs_as_it_always_has() {
    // Dot as it was built once, its bytes kept with the sound's goldens: a
    // change that moves its frames or its tones breaks every cartridge
    // already made. Kept once, never kept again.
    let file = include_bytes!("../../headless/tests/kept/dot.wasm");
    let buttons: Vec<u32> = (0..120)
        .map(|tick| match tick % 40 {
            0..10 => cartridge::button::RIGHT,
            10..20 => cartridge::button::DOWN,
            20 | 30 => cartridge::button::A,
            _ => 0,
        })
        .collect();
    let run = on_every_engine(file, 1, &buttons);
    assert_eq!(run.to_string(), "d09de795 b0e8de49 running 120");
}

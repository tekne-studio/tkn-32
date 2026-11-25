//! The cartridge built to WebAssembly and run as a runner runs it: it keeps
//! the contract, each pad's buttons move its dot and play its tone, well
//! within the budget, every engine at hand shows the same frames, and its
//! file says its cover and the pads it plays.
//! `built!` builds the crate it is written in, so a copy of this cart tests
//! itself as it is.

use cartridge::UPDATE_BUDGET;
use cartridge::button::{self, on};
use cartridge_test::built;
use cartridge_wasmi::Wasmi;

const SEED: u32 = 1;

/// Right for a second, then down, pressing A twice on the way; the second
/// pad left all along, pressing its A once.
fn buttons() -> Vec<u32> {
    (0..120)
        .map(|tick| match tick {
            0..60 => button::RIGHT,
            60 | 90 => button::DOWN | button::A,
            _ => button::DOWN,
        })
        .zip(0..)
        .map(|(first, tick)| match tick {
            30 => first | on(2, button::LEFT | button::A),
            _ => first | on(2, button::LEFT),
        })
        .collect()
}

#[test]
fn it_runs_moves_and_beeps_within_its_budget() {
    let mut cart = Wasmi::open(built!(), SEED);
    let (mut first, mut tones, mut peak) = (None, 0, 0);
    for (tick, held) in buttons().into_iter().enumerate() {
        tones += cart.update(held).len();
        assert_eq!(cart.stop(), None, "stopped at update {tick}");
        first.get_or_insert(*cart.screen());
        peak = peak.max(cart.spent());
    }
    assert!(
        Some(cart.screen()) != first.as_ref(),
        "the buttons moved nothing"
    );
    assert_eq!(tones, 3, "one tone a press of either pad's A");
    assert!(
        peak < UPDATE_BUDGET / 4,
        "the heaviest update took {peak} of {UPDATE_BUDGET}"
    );
}

#[test]
fn each_pad_moves_its_own_dot() {
    // Where the dot of `colour` lies: its first pixel, row by row.
    let at = |pixels: &[u8], colour: u8| pixels.iter().position(|&pixel| pixel == colour);
    let mut cart = Wasmi::open(built!(), SEED);
    cart.update(button::START);
    let still = cart.screen().to_vec();
    let moved = |held: u32| {
        let mut cart = Wasmi::open(built!(), SEED);
        cart.update(button::START);
        for _ in 0..10 {
            cart.update(held);
        }
        cart.screen().to_vec()
    };
    let (first, second) = (moved(button::RIGHT), moved(on(2, button::RIGHT)));
    // Dot's colours at seed 1: the first pad's 3, the second's 7.
    for (screen, colour, by) in [(&first, 3, 10), (&second, 7, 10)] {
        let (from, to) = (at(&still, colour).unwrap(), at(screen, colour).unwrap());
        assert_eq!(to - from, by, "colour {colour}");
    }
    assert_eq!(
        at(&first, 7),
        at(&still, 7),
        "the first pad moves no other dot"
    );
    assert_eq!(
        at(&second, 3),
        at(&still, 3),
        "the second pad moves no other dot"
    );
}

#[test]
fn every_engine_shows_the_same_frames() {
    let run = cartridge_test::on_every_engine(built!(), SEED, &buttons());
    assert_eq!((run.stop.as_str(), run.updates), ("running", 120), "{run}");
}

#[test]
fn its_file_carries_its_cover_and_its_first_screen_is_it() {
    let file = built!();
    let cover = cartridge_run::cover(file).expect("the release build keeps the section");
    assert_eq!(
        (cover.title(), cover.maker(), cover.year()),
        ("DOT", "TEKNE STUDIO", 2026)
    );
    let mut drawn = cartridge::Screen::new();
    cover.draw(&mut drawn);
    let mut cart = Wasmi::open(file, SEED);
    cart.update(0);
    assert!(
        cart.screen() == drawn.pixels(),
        "the first screen is the cover"
    );
    assert_eq!(*cart.palette(), cover.palette());
}

#[test]
fn its_file_says_it_plays_every_pad() {
    let prepared = cartridge_run::prepare(built!()).expect("it keeps the contract");
    assert_eq!(prepared.pads(), button::PADS);
}

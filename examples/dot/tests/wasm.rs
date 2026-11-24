//! The cartridge built to WebAssembly and run as a runner runs it: it keeps
//! the contract, its buttons move it and play its tone, well within the
//! budget, and every engine at hand shows the same frames. `built!` builds
//! the crate it is written in, so a copy of this cart tests itself as it is.

use cartridge::{UPDATE_BUDGET, button};
use cartridge_test::built;
use cartridge_wasmi::Wasmi;

const SEED: u32 = 1;

/// Right for a second, then down, pressing A twice on the way.
fn buttons() -> Vec<u32> {
    (0..120)
        .map(|tick| match tick {
            0..60 => button::RIGHT,
            60 | 90 => button::DOWN | button::A,
            _ => button::DOWN,
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
    assert_eq!(tones, 2, "one tone a press of A");
    assert!(
        peak < UPDATE_BUDGET / 4,
        "the heaviest update took {peak} of {UPDATE_BUDGET}"
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

//! rocks built to WebAssembly and run as a cabinet runs it: the same seed
//! and buttons give the same frames on every run, on wasmi, on every
//! browser engine at hand and natively, and no update runs past its budget,
//! in play and in the attract mode, where no button is held and the demo's
//! pilot flies.

use cartridge::{Buttons, Game, Screen, UPDATE_BUDGET, button};
use cartridge_test::built;
use cartridge_wasmi::Wasmi;
use rocks::Rocks;

/// A minute and a half of play.
const TICKS: u32 = 90 * cartridge::TICKS;
const SEED: u32 = 7;

/// No button for long enough to see the title, a whole demo and the title
/// again.
fn attract() -> Vec<u32> {
    vec![0; 45 * cartridge::TICKS as usize]
}

/// Starts, then turns, thrusts and fires in a pattern that meets rocks, and
/// presses start again whenever a game is over.
fn buttons(tick: u32) -> u32 {
    if tick < 30 {
        return 0;
    }
    let mut held = if tick % 200 < 4 { button::START } else { 0 };
    if tick % 12 < 3 {
        held |= button::A;
    }
    held |= match (tick / 40) % 6 {
        0 | 3 => button::RIGHT,
        1 => button::UP,
        4 => button::LEFT,
        _ => 0,
    };
    held
}

#[test]
fn the_same_seed_and_buttons_give_the_same_frames_on_wasmi_and_natively() {
    agree_on_wasmi_and_natively(&(0..TICKS).map(buttons).collect::<Vec<_>>());
}

#[test]
fn the_attract_mode_gives_the_same_frames_on_wasmi_and_natively() {
    agree_on_wasmi_and_natively(&attract());
}

/// Runs `buttons` on two runs on wasmi and natively, which agree to the
/// frame, the heaviest update well within the budget.
fn agree_on_wasmi_and_natively(buttons: &[u32]) {
    let file = built!();
    let mut runs = [Wasmi::open(file, SEED), Wasmi::open(file, SEED)];
    let mut native = Rocks::start(SEED);
    let mut screen = Screen::new();
    let mut peak = 0;
    for (tick, &held) in buttons.iter().enumerate() {
        native.update(Buttons(held), &mut screen);
        for cart in &mut runs {
            cart.update(held);
            assert_eq!(cart.stop(), None, "stopped at tick {tick}");
            assert!(
                cart.screen() == screen.pixels(),
                "tick {tick}: the screen on wasmi is not the native one"
            );
            assert_eq!(cart.palette(), &native.palette(), "tick {tick}");
            peak = peak.max(cart.spent());
        }
    }
    // The heaviest update left room to spare under the budget.
    assert!(
        peak < UPDATE_BUDGET / 4,
        "the heaviest update took {peak} of {UPDATE_BUDGET}"
    );
}

#[test]
fn the_same_seed_and_buttons_give_the_same_frames_on_every_engine() {
    agree_on_every_engine(&(0..TICKS).map(buttons).collect::<Vec<_>>());
}

#[test]
fn the_attract_mode_gives_the_same_frames_on_every_engine() {
    agree_on_every_engine(&attract());
}

#[test]
fn its_frames_and_tones_are_the_ones_kept() {
    // Rocks's frames and tones as they were first pinned: a change to the
    // contract or to `cart!` that moves them changes what every copy of
    // rocks already made shows.
    let run = agree_on_every_engine(&(0..TICKS).map(buttons).collect::<Vec<_>>());
    assert_eq!(
        run.to_string(),
        "8acec86a 15ccf266 running 5400 669ab0d8 check:"
    );
}

fn agree_on_every_engine(held: &[u32]) -> cartridge_test::Run {
    let run = cartridge_test::on_every_engine(built!(), SEED, held);
    assert_eq!(
        (run.stop.as_str(), run.updates),
        ("running", held.len() as u64),
        "{run}"
    );
    run
}

#[test]
fn the_built_file_carries_its_cover() {
    let file = built!();
    let cover = cartridge_run::cover(file).expect("the release build keeps the section");
    assert_eq!(
        (cover.title(), cover.maker(), cover.year()),
        ("ROCKS", "TEKNE STUDIO", 2026)
    );
    let (palette, screen) = Rocks::cover();
    assert_eq!(cover.palette(), palette);
    let mut drawn = Screen::new();
    cover.draw(&mut drawn);
    assert!(
        drawn.pixels() == screen.pixels(),
        "the file's cover is the game's drawing"
    );
}

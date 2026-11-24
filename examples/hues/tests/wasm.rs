//! Hues built to WebAssembly and run as a runner runs it: every engine at
//! hand shows the same frames in all 32 colours, and its file carries a
//! cover that is its first screen.

use cartridge::{PIXELS, UPDATE_BUDGET, button};
use cartridge_test::built;
use cartridge_wasmi::Wasmi;

const SEED: u32 = 3;

/// Right four times, A twice, left once.
fn buttons() -> Vec<u32> {
    (0..120)
        .map(|tick| match tick % 20 {
            0 if tick < 80 => button::RIGHT,
            10 if tick < 40 => button::A,
            5 if tick >= 100 => button::LEFT,
            _ => 0,
        })
        .collect()
}

#[test]
fn it_draws_in_all_32_colours_on_every_engine() {
    let run = cartridge_test::on_every_engine(built!(), SEED, &buttons());
    assert_eq!((run.stop.as_str(), run.updates), ("running", 120), "{run}");
    let mut cart = Wasmi::open(built!(), SEED);
    let mut peak = 0;
    for held in buttons() {
        cart.update(held);
        peak = peak.max(cart.spent());
    }
    // The last band is colour 31, the palette's last.
    assert_eq!(cart.screen()[PIXELS - 1], 31);
    let mut rgba = vec![0; PIXELS * 4];
    cart.rgba(&mut rgba);
    assert_eq!(rgba[rgba.len() - 4..], cart.palette()[31]);
    assert!(peak < UPDATE_BUDGET / 4, "the heaviest update took {peak}");
}

#[test]
fn its_file_carries_its_cover_and_its_first_screen_is_it() {
    let file = built!();
    let cover = cartridge_run::cover(file).expect("the release build keeps the section");
    assert_eq!(
        (cover.title(), cover.maker(), cover.year()),
        ("HUES", "TEKNE STUDIO", 2026)
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
    // The update that draws it, about 190,000 units: a tenth of the budget
    // at most, so a game may draw it on any update.
    assert!(
        cart.spent() < UPDATE_BUDGET / 10,
        "drawing the cover took {}",
        cart.spent()
    );
}

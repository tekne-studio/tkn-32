//! The cartridge built to WebAssembly and run as a runner runs it: A and B
//! play their moving tones in `env.tone`'s words, well within the budget,
//! and every engine at hand shows the same frames and keeps the same tones.

use cartridge::{UPDATE_BUDGET, button, wave};
use cartridge_run::Tone;
use cartridge_test::built;
use cartridge_wasmi::Wasmi;

const SEED: u32 = 1;

/// A at once, B a little after, both again after a second.
fn buttons() -> Vec<u32> {
    (0..120)
        .map(|tick| match tick % 60 {
            0 => button::A,
            20 => button::B,
            _ => 0,
        })
        .collect()
}

#[test]
fn a_and_b_play_sounds_that_move_within_its_budget() {
    let mut cart = Wasmi::open(built!(), SEED);
    let (mut played, mut peak, mut screens) = (Vec::<Tone>::new(), 0, Vec::new());
    for (tick, held) in buttons().into_iter().enumerate() {
        played.extend(cart.update(held));
        assert_eq!(cart.stop(), None, "stopped at update {tick}");
        peak = peak.max(cart.spent());
        screens.push(*cart.screen());
    }
    let boom = Tone::new(0, 160, 250, wave::TRIANGLE as u8, 45, 255, 0);
    let whoosh = Tone::new(1, 6000, 600, wave::NOISE as u8, 800, 220, 0);
    assert_eq!(played, [boom, whoosh, boom, whoosh]);
    assert!(screens[1] != screens[10], "the boom is drawn as it falls");
    assert!(
        screens[21] != screens[40],
        "the whoosh is drawn as it falls"
    );
    assert!(
        screens[57] == screens[59],
        "the pads rest once both are over"
    );
    assert!(
        peak < UPDATE_BUDGET / 4,
        "the heaviest update took {peak} of {UPDATE_BUDGET}"
    );
}

#[test]
fn every_engine_shows_the_same_frames_and_tones() {
    let run = cartridge_test::on_every_engine(built!(), SEED, &buttons());
    assert_eq!((run.stop.as_str(), run.updates), ("running", 120), "{run}");
}

//! The chip's samples are the same bits on every platform. This runs
//! natively and on wasm32 under node (`.cargo/config.toml`):
//! `cargo test -p chip --target wasm32-unknown-unknown`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use cartridge::{LOUDEST, wave};
use chip::Chip;

const RATE: f32 = 48_000.0;

/// FNV-1a over the samples' bits.
fn fingerprint(samples: &[f32]) -> u64 {
    samples.iter().fold(0xCBF2_9CE4_8422_2325, |hash, sample| {
        sample
            .to_bits()
            .to_le_bytes()
            .iter()
            .fold(hash, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01B3)
            })
    })
}

#[test]
fn the_chip_gives_the_same_samples_on_every_platform() {
    let mut chip = Chip::new(RATE);
    let mut out = vec![0.0; RATE as usize];
    // Every wave at once, a slide, a fade, noise through its low-pass.
    chip.play(0, 440, 600, wave::SQUARE, 880, LOUDEST, 90);
    chip.play(1, 220, 600, wave::TRIANGLE, 220, 200, 200);
    chip.play(2, 1_760, 600, wave::SAW, 110, 120, LOUDEST);
    chip.play(3, 3_000, 600, wave::NOISE, 400, LOUDEST, 0);
    chip.render(&mut out[..24_000]);
    // A tone replacing a sounding one, at another loudness and wave.
    chip.play(1, 330, 300, wave::SQUARE, 330, 60, 60);
    chip.render(&mut out[24_000..]);
    assert_eq!(
        fingerprint(&out),
        0xF9B9_04E5_4530_AFC4,
        "a change to how the chip renders changes this on purpose"
    );
}

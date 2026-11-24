//! The headless runner's pieces, for its binary and its tests: a script of
//! buttons ([`Script`]) and a cart's tones heard through the arcade's chip
//! ([`Heard`]).

mod script;

use cartridge::TICKS;
use cartridge_run::Tone;
use chip::Chip;

pub use script::Script;

/// The samples a second the tones are heard at: 800 an update.
pub const RATE: u32 = 48_000;

/// A cart's tones heard through the chip, update by update, as the
/// workbench's worklet hears them: each update's tones played on its first
/// sample, in the order played, then the update's samples rendered.
pub struct Heard {
    chip: Chip,
    samples: Vec<f32>,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            chip: Chip::new(RATE as f32),
            samples: Vec::new(),
        }
    }
}

impl Heard {
    /// Hears one update: what it played, then its samples.
    pub fn update(&mut self, played: &[Tone]) {
        for tone in played {
            self.chip.play(
                u32::from(tone.channel),
                tone.hz,
                tone.ms,
                u32::from(tone.wave),
                tone.to_hz,
                u32::from(tone.loud),
                u32::from(tone.to_loud),
            );
        }
        self.render();
    }

    /// Every sample heard, a tone still sounding when the run ends heard to
    /// its end.
    pub fn end(mut self) -> Vec<f32> {
        while self.chip.sounding().contains(&true) {
            self.render();
        }
        self.samples
    }

    fn render(&mut self) {
        let from = self.samples.len();
        self.samples.resize(from + (RATE / TICKS) as usize, 0.0);
        self.chip.render(&mut self.samples[from..]);
    }
}

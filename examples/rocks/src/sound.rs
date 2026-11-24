//! What the game plays: a sweep on each of three channels, a pitch that
//! slides a step a tick, and a tune of notes on the fourth.

use cartridge::{TICKS, tone, wave};

/// The channels, by what plays on them.
pub const SHOTS: usize = 0;
pub const ENGINE: usize = 1;
pub const BLASTS: usize = 2;
const TUNE: u32 = 3;

/// A tone held this long is replaced by the next tick's before it ends.
const HELD_MS: u32 = 40;

#[derive(Clone, Copy, Default)]
struct Sweep {
    hz: i32,
    step: i32,
    left: u32,
    wave: u32,
}

#[derive(Clone, Copy, Default)]
pub struct Sound {
    sweeps: [Sweep; 3],
    tune: [(u16, u8); 6],
    notes: usize,
    next: usize,
    held: u8,
}

impl Sound {
    /// Slides `channel` from `from` to `to` hertz over `ticks`.
    pub fn sweep(&mut self, channel: usize, from: i32, to: i32, ticks: u32, wave: u32) {
        let ticks = ticks.max(1);
        self.sweeps[channel] = Sweep {
            hz: from,
            step: (to - from) / ticks as i32,
            left: ticks,
            wave,
        };
    }

    /// Plays `notes`, each a pitch and its length in ticks, replacing the tune
    /// that plays.
    pub fn tune(&mut self, notes: &[(u16, u8)]) {
        let notes = &notes[..notes.len().min(self.tune.len())];
        self.tune[..notes.len()].copy_from_slice(notes);
        (self.notes, self.next, self.held) = (notes.len(), 0, 0);
    }

    /// One tick's tones.
    pub fn play(&mut self) {
        for (channel, sweep) in (0..).zip(self.sweeps.iter_mut()) {
            if sweep.left > 0 {
                tone(channel, sweep.hz.max(1) as u32, HELD_MS, sweep.wave);
                sweep.hz += sweep.step;
                sweep.left -= 1;
            }
        }
        if self.held > 0 {
            self.held -= 1;
        } else if self.next < self.notes {
            let (hz, ticks) = self.tune[self.next];
            // As long as its ticks last, to the millisecond above.
            let ms = (u32::from(ticks) * 1000).div_ceil(TICKS);
            tone(TUNE, u32::from(hz), ms, wave::TRIANGLE);
            self.held = ticks.saturating_sub(1);
            self.next += 1;
        }
    }
}

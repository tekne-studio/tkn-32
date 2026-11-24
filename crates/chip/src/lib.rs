//! The arcade's sound chip: what a cartridge plays through `env.tone`,
//! heard. Four channels, each playing one tone at a time; a tone given to
//! a channel replaces what it played, its phase carried on, and whatever
//! step lies between the last sample heard and the new tone's first, in
//! loudness or in wave, is eased out over the attack, so the change never
//! clicks.
//!
//! A tone is the contract's words ([`Chip::play`]): a wave, a pitch that
//! slides by equal ratios to its end pitch, a loudness that goes in a
//! straight line to its end loudness, a length. The chip renders it one
//! sample at a time ([`Chip::render`]), so a tone starts on the sample it
//! is given and a page, a cabinet or a test hears the same thing:
//!
//! - square and saw are band-limited by polyBLEP, so a high note keeps its
//!   pitch instead of folding into whistles;
//! - triangle is plain, its harmonics already fall fast;
//! - noise is a 15-bit linear-feedback register through a two-pole
//!   low-pass whose edge is the tone's pitch: the pitch is how high the
//!   noise reaches, and a slide sweeps it.
//!
//! Only `libm` math, so the samples are the same bits on native and WASM.

#![no_std]

use cartridge::{CHANNELS, LOUDEST, wave};
use core::f32::consts::PI;
use libm::{expf, logf, tanf};

/// The pitches a tone is heard within, whatever the cartridge asks: below,
/// a buzz no speaker plays; above, past what an output holds cleanly.
const PITCHES_HZ: (u32, u32) = (20, 12_000);
/// The lengths a tone sounds within: shorter is a click, and a held note
/// is struck again by its cartridge.
const LENGTHS_MS: (u32, u32) = (10, 2_000);
/// How quickly a tone comes in and goes out, inside its length: soon
/// enough to sound struck, slow enough not to click.
const ATTACK_S: f32 = 0.002;
const RELEASE_S: f32 = 0.008;
/// How loud one channel is at its loudest: the four at once stay within one.
const GAIN: f32 = 0.25;
/// The noise low-pass's resonance: a gentle knee, no peak.
const NOISE_Q: f32 = 0.7;

/// Four channels at one sample rate.
pub struct Chip {
    rate: f32,
    channels: [Channel; CHANNELS as usize],
}

impl Chip {
    /// A silent chip rendering `rate` samples a second.
    pub fn new(rate: f32) -> Chip {
        Chip {
            rate,
            channels: core::array::from_fn(|_| Channel::default()),
        }
    }

    /// Plays a tone in `env.tone`'s words, from the next sample rendered.
    /// `hz` 0, or both loudnesses 0, lets the channel's tone go out; a
    /// channel or a wave the contract has not is ignored.
    #[allow(clippy::too_many_arguments)]
    pub fn play(
        &mut self,
        channel: u32,
        hz: u32,
        ms: u32,
        wave: u32,
        to_hz: u32,
        loud: u32,
        to_loud: u32,
    ) {
        if channel >= CHANNELS || wave >= wave::COUNT {
            return;
        }
        let rate = self.rate;
        let playing = &mut self.channels[channel as usize];
        let release = (RELEASE_S * rate) as u32;
        if hz == 0 || loud == 0 && to_loud == 0 {
            playing.left = playing.left.min(release);
            return;
        }
        let pitch = |hz: u32| hz.clamp(PITCHES_HZ.0, PITCHES_HZ.1) as f32;
        let (hz, to_hz) = (pitch(hz), pitch(to_hz));
        let length = (ms.clamp(LENGTHS_MS.0, LENGTHS_MS.1) as f32 / 1000.0 * rate) as u32;
        let level = |loud: u32| loud.min(LOUDEST) as f32 / LOUDEST as f32;
        *playing = Channel {
            wave,
            hz,
            glide: expf(logf(to_hz / hz) / length as f32),
            level: level(loud),
            slope: (level(to_loud) - level(loud)) / length as f32,
            left: length,
            release,
            rise: 1.0 / (ATTACK_S * rate),
            mend: true,
            ..*playing
        };
    }

    /// The next `out.len()` samples, the four channels mixed.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        for channel in &mut self.channels {
            if channel.left == 0 {
                continue;
            }
            for sample in out.iter_mut() {
                *sample += channel.next(self.rate);
                if channel.left == 0 {
                    break;
                }
            }
        }
    }

    /// Whether each channel is sounding.
    pub fn sounding(&self) -> [bool; CHANNELS as usize] {
        core::array::from_fn(|at| self.channels[at].left > 0)
    }
}

/// One channel: the tone it plays and where it is in it.
#[derive(Clone, Copy)]
struct Channel {
    wave: u32,
    /// Where the wave is in its cycle, `0..1`.
    phase: f32,
    hz: f32,
    /// What the pitch is multiplied by each sample.
    glide: f32,
    /// The tone's loudness, `0..=1`, and what it moves by each sample.
    level: f32,
    slope: f32,
    /// How far the tone has come in and gone out, `0..=1`.
    envelope: f32,
    rise: f32,
    /// Samples left of the tone, and the last ones it goes out over.
    left: u32,
    release: u32,
    /// The noise register and its low-pass's two states.
    noise: u32,
    low: [f32; 2],
    /// The last sample heard; whether a new tone has yet to meet it; the
    /// step between them still to ease out, and what it eases by a sample.
    last: f32,
    mend: bool,
    ease: f32,
    ease_by: f32,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            wave: wave::SQUARE,
            phase: 0.0,
            hz: 0.0,
            glide: 1.0,
            level: 0.0,
            slope: 0.0,
            envelope: 0.0,
            rise: 0.0,
            left: 0,
            release: 0,
            noise: 1,
            low: [0.0; 2],
            last: 0.0,
            mend: false,
            ease: 0.0,
            ease_by: 0.0,
        }
    }
}

impl Channel {
    /// One sample of the tone, and the tone moved on by one.
    fn next(&mut self, rate: f32) -> f32 {
        let step = (self.hz / rate).min(0.5);
        let shape = match self.wave {
            wave::SQUARE => {
                let half = (self.phase + 0.5) % 1.0;
                let level = if self.phase < 0.5 { 1.0 } else { -1.0 };
                level + blep(self.phase, step) - blep(half, step)
            }
            wave::TRIANGLE => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            wave::SAW => 2.0 * self.phase - 1.0 - blep(self.phase, step),
            _ => self.noise(rate),
        };
        self.phase = (self.phase + step) % 1.0;
        self.envelope = match self.left <= self.release {
            true => self
                .envelope
                .min(self.left as f32 / self.release.max(1) as f32),
            false => (self.envelope + self.rise).min(1.0),
        };
        let mut sample = shape * self.envelope * self.level * GAIN;
        if self.mend {
            self.mend = false;
            self.ease = self.last - sample;
            self.ease_by = self.ease * self.rise;
        }
        if self.ease != 0.0 {
            sample += self.ease;
            let eased = self.ease - self.ease_by;
            // Eased past zero: the step is gone.
            self.ease = if eased * self.ease > 0.0 { eased } else { 0.0 };
        }
        self.last = sample;
        self.hz *= self.glide;
        self.level = (self.level + self.slope).clamp(0.0, 1.0);
        self.left -= 1;
        sample
    }

    /// White noise from the register, through a two-pole low-pass at the
    /// pitch (a state-variable filter, trapezoidal).
    fn noise(&mut self, rate: f32) -> f32 {
        let bit = (self.noise ^ (self.noise >> 1)) & 1;
        self.noise = (self.noise >> 1) | (bit << 14);
        let white = if self.noise & 1 == 1 { 1.0 } else { -1.0 };
        let g = tanf(PI * self.hz.min(0.45 * rate) / rate);
        let k = 1.0 / NOISE_Q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let (a2, a3) = (g * a1, g * g * a1);
        let [ic1, ic2] = self.low;
        let v3 = white - ic2;
        let v1 = a1 * ic1 + a2 * v3;
        let v2 = ic2 + a2 * ic1 + a3 * v3;
        self.low = [2.0 * v1 - ic1, 2.0 * v2 - ic2];
        v2
    }
}

/// The polyBLEP step: what takes the corner off a jump at `phase` 0, for
/// a wave that moves `step` a sample.
fn blep(phase: f32, step: f32) -> f32 {
    if phase < step {
        let t = phase / step;
        2.0 * t - t * t - 1.0
    } else if phase > 1.0 - step {
        let t = (phase - 1.0) / step;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    const RATE: f32 = 48_000.0;

    fn rendered(chip: &mut Chip, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * RATE) as usize];
        chip.render(&mut out);
        out
    }

    /// How many times the samples cross zero going up, a second.
    fn pitch_of(samples: &[f32]) -> f32 {
        let ups = samples
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        ups as f32 * RATE / samples.len() as f32
    }

    #[test]
    fn a_chip_given_nothing_is_silent() {
        let mut chip = Chip::new(RATE);
        assert!(rendered(&mut chip, 0.1).iter().all(|&s| s == 0.0));
    }

    #[test]
    fn a_tone_sounds_its_pitch_for_its_length_then_stops() {
        for shape in [wave::SQUARE, wave::TRIANGLE, wave::SAW] {
            let mut chip = Chip::new(RATE);
            chip.play(0, 440, 500, shape, 440, LOUDEST, LOUDEST);
            let out = rendered(&mut chip, 0.6);
            let heard = pitch_of(&out[2_000..22_000]);
            assert!((heard - 440.0).abs() < 5.0, "wave {shape}: {heard} hz");
            assert!(
                out[24_000..].iter().all(|&s| s == 0.0),
                "wave {shape} rings past its length"
            );
            assert!(out.iter().all(|&s| s.abs() <= GAIN * 1.1));
            assert!(!chip.sounding()[0]);
        }
    }

    #[test]
    fn a_slide_ends_at_its_end_pitch_and_a_fade_at_its_end_loudness() {
        let mut chip = Chip::new(RATE);
        chip.play(1, 880, 1_000, wave::SQUARE, 110, LOUDEST, 0);
        let out = rendered(&mut chip, 1.0);
        let start = pitch_of(&out[..4_800]);
        let end = pitch_of(&out[43_200..48_000]);
        assert!(start > 700.0 && end < 140.0, "{start} hz to {end} hz");
        let peak = |samples: &[f32]| samples.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
        assert!(peak(&out[..4_800]) > 0.2 && peak(&out[43_200..46_000]) < 0.03);
    }

    #[test]
    fn noise_reaches_as_high_as_its_pitch() {
        let crossings = |hz| {
            let mut chip = Chip::new(RATE);
            chip.play(3, hz, 500, wave::NOISE, hz, LOUDEST, LOUDEST);
            pitch_of(&rendered(&mut chip, 0.5))
        };
        let (low, high) = (crossings(200), crossings(6_000));
        assert!(high > low * 4.0, "{low} and {high} crossings a second");
    }

    #[test]
    fn a_tone_replacing_another_never_jumps() {
        // Over a sounding tone, a triangle of another pitch, loudness or
        // wave, struck at several points of the cycle. A triangle's own
        // step at 330 Hz and full loudness is about 0.007 a sample; a click
        // is a jump many times that.
        let changes = [
            ((wave::TRIANGLE, 220, 90), (330, LOUDEST)),
            ((wave::TRIANGLE, 220, LOUDEST), (220, 90)),
            ((wave::SQUARE, 220, 200), (220, 200)),
            ((wave::SAW, 330, 200), (220, 90)),
        ];
        for ((from, from_hz, from_loud), (hz, loud)) in changes {
            for after in [0.1, 0.1013, 0.1027, 0.104] {
                let mut chip = Chip::new(RATE);
                chip.play(0, from_hz, 1_000, from, from_hz, from_loud, from_loud);
                let mut out = rendered(&mut chip, after);
                chip.play(0, hz, 1_000, wave::TRIANGLE, hz, loud, loud);
                out.extend(rendered(&mut chip, 0.01));
                let from_last = out.len() - 481;
                let jump = out[from_last..]
                    .windows(2)
                    .fold(0.0_f32, |m, w| m.max((w[1] - w[0]).abs()));
                assert!(
                    jump < 0.01,
                    "wave {from} to a triangle at {hz} Hz, loud {loud}, after {after} s: {jump}"
                );
            }
        }
    }

    #[test]
    fn any_numbers_the_contract_passes_are_held_in_bounds() {
        // Never longer than two seconds, never shorter than a click.
        let heard = |ms| {
            let mut chip = Chip::new(RATE);
            chip.play(0, 440, ms, wave::SAW, 440, LOUDEST, LOUDEST);
            let out = rendered(&mut chip, 2.5);
            out.iter()
                .rposition(|&s| s != 0.0)
                .map_or(0, |last| last + 1)
        };
        assert_eq!(heard(u32::MAX), 2 * RATE as usize);
        assert_eq!(heard(0), RATE as usize / 100);
        // Never above what an output holds, however loud it asks to be.
        let mut chip = Chip::new(RATE);
        chip.play(0, u32::MAX, 100, wave::SQUARE, u32::MAX, u32::MAX, u32::MAX);
        let shrill = rendered(&mut chip, 0.1);
        assert!(shrill.iter().all(|&s| s.abs() <= GAIN * 1.1));
        assert!((pitch_of(&shrill[500..]) - 12_000.0).abs() < 200.0);
    }

    #[test]
    fn hz_zero_lets_a_tone_go_out_and_what_the_contract_lacks_is_ignored() {
        let mut chip = Chip::new(RATE);
        chip.play(2, 440, 2_000, wave::SAW, 440, LOUDEST, LOUDEST);
        rendered(&mut chip, 0.1);
        chip.play(2, 0, 0, 0, 0, 0, 0);
        assert!(rendered(&mut chip, 0.01).last() == Some(&0.0));
        chip.play(4, 440, 100, wave::SQUARE, 440, LOUDEST, LOUDEST);
        chip.play(0, 440, 100, wave::COUNT, 440, LOUDEST, LOUDEST);
        assert_eq!(chip.sounding(), [false; 4]);
    }
}

//! Boom, two pads of tones that move (`cartridge::tone_to`): A strikes an
//! 808 boom, its pitch falling and its loudness dying within a quarter of a
//! second; B a whoosh, noise whose edge falls from bright to dull. Each pad
//! draws its tone as it plays: on a scale of octaves a slide by equal
//! ratios is a straight line, so the pitch, or the edge, falls at an even
//! pace down its pad.

#![cfg_attr(target_arch = "wasm32", no_std)]

use cartridge::{Buttons, Game, Palette, Screen, TICKS, button, tone_to, wave};

cartridge::cart!(Boom);

/// The boom: a triangle from 160 Hz to 45 Hz, from the loudest to silence.
const BOOM: PadTone = PadTone {
    channel: 0,
    wave: wave::TRIANGLE,
    hz: 160,
    to_hz: 45,
    ms: 250,
    loud: 255,
    to_loud: 0,
};

/// The whoosh: noise whose edge falls from 6000 Hz to 800 Hz as it fades.
const WHOOSH: PadTone = PadTone {
    channel: 1,
    wave: wave::NOISE,
    hz: 6000,
    to_hz: 800,
    ms: 600,
    loud: 220,
    to_loud: 0,
};

/// A pad's place: its left edge, and the top and height every pad shares.
const PAD_TOP: i32 = 22;
const PAD_HEIGHT: i32 = 88;
const PAD_WIDTH: i32 = 68;
const PADS: [i32; 2] = [8, 84];

mod colour {
    pub const GROUND: u8 = 0;
    pub const WORDS: u8 = 1;
    pub const PAD: u8 = 2;
    pub const EDGE: u8 = 3;
    /// The boom from its loudest to its last, four steps.
    pub const BOOM: [u8; 4] = [4, 5, 6, 7];
    /// The whoosh's noise, bright and dull.
    pub const NOISE: [u8; 2] = [8, 9];
}

#[rustfmt::skip]
const PALETTE: Palette = [
    [12, 12, 20, 255],    // 0 the ground
    [236, 236, 244, 255], // 1 the words
    [24, 26, 40, 255],    // 2 a pad
    [70, 74, 104, 255],   // 3 a pad's edge
    [255, 236, 170, 255], // 4 the boom, loudest
    [255, 170, 80, 255],  // 5
    [220, 90, 60, 255],   // 6
    [120, 50, 50, 255],   // 7 the boom, last
    [170, 220, 255, 255], // 8 noise, bright
    [80, 120, 200, 255],  // 9 noise, dull
    // 10 to 31: unused, black.
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
];

/// One tone a pad strikes, in `tone_to`'s words.
struct PadTone {
    channel: u32,
    wave: u32,
    hz: u32,
    to_hz: u32,
    ms: u32,
    loud: u32,
    to_loud: u32,
}

impl PadTone {
    fn strike(&self) {
        let PadTone {
            channel,
            wave,
            hz,
            to_hz,
            ms,
            loud,
            to_loud,
        } = *self;
        tone_to(channel, hz, ms, wave, to_hz, loud, to_loud);
    }

    /// The updates it lasts.
    const fn updates(&self) -> u32 {
        self.ms * TICKS / 1000
    }

    /// How far along it is after `age` updates, out of `of`: 0 at its
    /// strike, `of` once over.
    fn along(&self, age: u32, of: i32) -> i32 {
        (age.min(self.updates()) * of as u32 / self.updates()) as i32
    }
}

/// A pad: how many updates ago it was struck, none before the first.
#[derive(Clone, Copy)]
struct Pad {
    age: Option<u32>,
}

impl Pad {
    fn strike(&mut self, tone: &PadTone) {
        tone.strike();
        self.age = Some(0);
    }

    /// Its age while its tone plays.
    fn playing(&self, tone: &PadTone) -> Option<u32> {
        self.age.filter(|&age| age < tone.updates())
    }

    fn tick(&mut self) {
        self.age = self.age.map(|age| age.saturating_add(1));
    }
}

pub struct Boom {
    pads: [Pad; 2],
    before: Buttons,
    /// Each update's own number: the noise's speckle moves with it.
    tick: u32,
}

impl Game for Boom {
    fn start(seed: u32) -> Boom {
        Boom {
            pads: [Pad { age: None }; 2],
            before: Buttons(0),
            tick: seed,
        }
    }

    fn update(&mut self, buttons: Buttons, screen: &mut Screen) {
        self.pads.iter_mut().for_each(Pad::tick);
        let pressed = buttons.pressed_since(self.before);
        if pressed.held(button::A) {
            self.pads[0].strike(&BOOM);
        }
        if pressed.held(button::B) {
            self.pads[1].strike(&WHOOSH);
        }
        self.before = buttons;
        self.tick = self.tick.wrapping_add(1);

        screen.clear(colour::GROUND);
        screen.text(PADS[0], 6, "A  BOOM", colour::WORDS, 1);
        screen.text(PADS[1], 6, "B  WHOOSH", colour::WORDS, 1);
        for left in PADS {
            screen.fill(left, PAD_TOP, PAD_WIDTH, PAD_HEIGHT, colour::PAD);
            screen.rect(left, PAD_TOP, PAD_WIDTH, PAD_HEIGHT, colour::EDGE);
        }
        if let Some(age) = self.pads[0].playing(&BOOM) {
            draw_boom(screen, age);
        }
        if let Some(age) = self.pads[1].playing(&WHOOSH) {
            draw_whoosh(screen, age, self.tick);
        }
    }

    fn palette(&self) -> Palette {
        PALETTE
    }
}

/// The boom: a square as wide as it is loud, at the height of its pitch.
fn draw_boom(screen: &mut Screen, age: u32) {
    let side = 4 + (PAD_WIDTH - 12) * (64 - BOOM.along(age, 64)) / 64;
    let fall = BOOM.along(age, PAD_HEIGHT - side - 4);
    let shade = colour::BOOM[BOOM.along(age, colour::BOOM.len() as i32 - 1) as usize];
    let x = PADS[0] + (PAD_WIDTH - side) / 2;
    screen.fill(x, PAD_TOP + 2 + fall, side, side, shade);
}

/// The whoosh: noise from the pad's foot up to its edge, the edge falling,
/// the speckle thinning as it fades.
fn draw_whoosh(screen: &mut Screen, age: u32, tick: u32) {
    let inside = PAD_HEIGHT - 4;
    let top = PAD_TOP + 2 + WHOOSH.along(age, inside - 4);
    let thin = 2 + WHOOSH.along(age, 6) as u32;
    let bottom = PAD_TOP + 2 + inside;
    for y in top..bottom {
        for x in PADS[1] + 2..PADS[1] + PAD_WIDTH - 2 {
            let speck = scatter(x as u32, y as u32, tick);
            if speck % thin == 0 {
                let shade = colour::NOISE[usize::from(y - top > (bottom - top) / 2)];
                screen.pixel(x, y, shade);
            }
        }
    }
}

/// A number that looks random for each place and update: a few rounds of
/// a multiply and a shift.
fn scatter(x: u32, y: u32, tick: u32) -> u32 {
    let mut n = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ tick;
    n ^= n >> 15;
    n = n.wrapping_mul(0x2C1B_3C6D);
    n ^ (n >> 12)
}

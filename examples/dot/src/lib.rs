//! Dot, the smallest whole cartridge: its cover first, then a dot each pad
//! moves, a beep and the next colour when its A is pressed. The guide to
//! making a cartridge starts from it, and a game of your own starts as a copy
//! of it.

#![cfg_attr(target_arch = "wasm32", no_std)]

use cartridge::button::{self, PADS};
use cartridge::{Buttons, Game, HEIGHT, Palette, Screen, WIDTH, tone, wave};

cartridge::cart!(Dot);
// cover.png, made the file's cover by build.rs, and `COVER` to draw it.
cartridge::cover!();
// A dot each pad: a host seats as many people as the console has pads.
cartridge::pads!(PADS);

/// The dot's side in pixels.
const SIDE: i32 = 4;

#[rustfmt::skip]
const PALETTE: Palette = [
    [16, 18, 28, 255],    // 0 the ground
    [236, 236, 244, 255], // 1 the words
    [255, 77, 106, 255],  // 2 red
    [255, 160, 64, 255],  // 3 orange
    [255, 220, 90, 255],  // 4 yellow
    [140, 230, 110, 255], // 5 green
    [80, 210, 200, 255],  // 6 teal
    [90, 150, 255, 255],  // 7 blue
    [170, 120, 255, 255], // 8 violet
    [255, 130, 210, 255], // 9 pink
    [200, 200, 210, 255], // 10 silver
    [150, 110, 80, 255],  // 11 brown
    [60, 64, 90, 255],    // 12 slate
    [110, 116, 150, 255], // 13 dim
    [30, 120, 90, 255],   // 14 moss
    [255, 255, 255, 255], // 15 white
    // 16 to 31: unused, black.
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
];

/// The dot's colours, in the order A steps through them.
const COLOURS: [u8; 8] = [2, 3, 4, 5, 6, 7, 8, 9];

pub struct Dot {
    /// Whether the cover still shows: until the first button of any pad.
    covered: bool,
    /// Whether the cover was drawn: once, since the screen keeps what the
    /// last update left.
    cover_drawn: bool,
    /// A dot each pad moves, the first pad's first.
    dots: [Spot; PADS as usize],
    before: Buttons,
}

/// Where a dot is, and its colour.
struct Spot {
    x: i32,
    y: i32,
    colour: usize,
}

impl Game for Dot {
    fn start(seed: u32) -> Dot {
        // The seed is a cartridge's only chance: here, the first colours.
        let y = (HEIGHT as i32 - SIDE) / 2;
        let dots = core::array::from_fn(|pad| Spot {
            x: (WIDTH as i32 - SIDE) * (pad as i32 + 1) / (PADS as i32 + 1),
            y,
            colour: (seed as usize + pad * COLOURS.len() / 2) % COLOURS.len(),
        });
        Dot {
            covered: true,
            cover_drawn: false,
            dots,
            before: Buttons(0),
        }
    }

    fn update(&mut self, buttons: Buttons, screen: &mut Screen) {
        self.covered &= !buttons.any();
        if self.covered {
            // An update that draws nothing costs next to nothing.
            if !core::mem::replace(&mut self.cover_drawn, true) {
                COVER.draw(screen);
            }
            return;
        }
        for (channel, dot) in (0..).zip(&mut self.dots) {
            // Each pad's buttons read as the first pad's: one dot's code
            // moves them all, and each pad beeps on a channel of its own.
            let pad = channel + 1;
            let (held, before) = (buttons.pad(pad), self.before.pad(pad));
            let axis = |less, more| i32::from(held.held(more)) - i32::from(held.held(less));
            dot.x = (dot.x + axis(button::LEFT, button::RIGHT)).clamp(0, WIDTH as i32 - SIDE);
            dot.y = (dot.y + axis(button::UP, button::DOWN)).clamp(0, HEIGHT as i32 - SIDE);
            if held.pressed_since(before).held(button::A) {
                dot.colour = (dot.colour + 1) % COLOURS.len();
                tone(channel, 440 + 55 * dot.colour as u32, 80, wave::SQUARE);
            }
        }
        self.before = buttons;

        screen.clear(0);
        screen.text(4, 4, "EACH PAD MOVES A DOT, A BEEPS", 1, 1);
        for dot in &self.dots {
            screen.fill(dot.x, dot.y, SIDE, SIDE, COLOURS[dot.colour]);
        }
    }

    fn palette(&self) -> Palette {
        if self.covered {
            COVER.palette()
        } else {
            PALETTE
        }
    }
}

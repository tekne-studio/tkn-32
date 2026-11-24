//! Dot, the smallest whole cartridge: its cover first, then a dot the arrows
//! move, a beep and the next colour when A is pressed. The guide to making a
//! cartridge starts from it, and a game of your own starts as a copy of it.

#![cfg_attr(target_arch = "wasm32", no_std)]

use cartridge::{Buttons, Game, HEIGHT, Palette, Screen, WIDTH, button, tone, wave};

cartridge::cart!(Dot);
// cover.png, made the file's cover by build.rs, and `COVER` to draw it.
cartridge::cover!();

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
    /// Whether the cover still shows: until the first button.
    covered: bool,
    /// Whether the cover was drawn: once, since the screen keeps what the
    /// last update left.
    cover_drawn: bool,
    x: i32,
    y: i32,
    colour: usize,
    before: Buttons,
}

impl Game for Dot {
    fn start(seed: u32) -> Dot {
        // The seed is a cartridge's only chance: here, the first colour.
        let colour = seed as usize % COLOURS.len();
        let (x, y) = ((WIDTH as i32 - SIDE) / 2, (HEIGHT as i32 - SIDE) / 2);
        Dot {
            covered: true,
            cover_drawn: false,
            x,
            y,
            colour,
            before: Buttons(0),
        }
    }

    fn update(&mut self, buttons: Buttons, screen: &mut Screen) {
        self.covered &= buttons.0 == 0;
        if self.covered {
            // An update that draws nothing costs next to nothing.
            if !core::mem::replace(&mut self.cover_drawn, true) {
                COVER.draw(screen);
            }
            return;
        }
        let axis = |less, more| i32::from(buttons.held(more)) - i32::from(buttons.held(less));
        self.x = (self.x + axis(button::LEFT, button::RIGHT)).clamp(0, WIDTH as i32 - SIDE);
        self.y = (self.y + axis(button::UP, button::DOWN)).clamp(0, HEIGHT as i32 - SIDE);
        if buttons.pressed_since(self.before).held(button::A) {
            self.colour = (self.colour + 1) % COLOURS.len();
            tone(0, 440 + 55 * self.colour as u32, 80, wave::SQUARE);
        }
        self.before = buttons;

        screen.clear(0);
        screen.text(4, 4, "ARROWS MOVE, A BEEPS", 1, 1);
        screen.fill(self.x, self.y, SIDE, SIDE, COLOURS[self.colour]);
    }

    fn palette(&self) -> Palette {
        if self.covered {
            COVER.palette()
        } else {
            PALETTE
        }
    }
}

//! Hues, a game of all 32 colours: its cover, a dusk painted in 32, then
//! its palette as 32 bands, the arrows picking one and A sounding it. The
//! guide's example of a palette used whole.

#![cfg_attr(target_arch = "wasm32", no_std)]

use cartridge::{Buttons, COLOURS, Game, HEIGHT, Palette, Screen, WIDTH, button, tone, wave};

cartridge::cart!(Hues);
// cover.png, made the file's cover by build.rs, and `COVER` to draw it.
cartridge::cover!();

/// A band's width: the screen's, shared by every colour.
const BAND: i32 = WIDTH as i32 / COLOURS as i32;

pub struct Hues {
    /// Whether the cover still shows: until the first button.
    covered: bool,
    /// Whether the cover was drawn: once, since the screen keeps what the
    /// last update left.
    cover_drawn: bool,
    /// The band picked.
    picked: usize,
    before: Buttons,
}

impl Game for Hues {
    fn start(seed: u32) -> Hues {
        Hues {
            covered: true,
            cover_drawn: false,
            picked: seed as usize % COLOURS,
            before: Buttons(0),
        }
    }

    fn update(&mut self, buttons: Buttons, screen: &mut Screen) {
        self.covered &= !buttons.any();
        if self.covered {
            if !core::mem::replace(&mut self.cover_drawn, true) {
                COVER.draw(screen);
            }
            return;
        }
        let pressed = buttons.pressed_since(self.before);
        self.before = buttons;
        if pressed.held(button::RIGHT) {
            self.picked = (self.picked + 1) % COLOURS;
        }
        if pressed.held(button::LEFT) {
            self.picked = (self.picked + COLOURS - 1) % COLOURS;
        }
        if pressed.held(button::A) {
            tone(0, 220 + 20 * self.picked as u32, 120, wave::TRIANGLE);
        }

        // Every colour a band, indices 0 to 31.
        for colour in 0..COLOURS {
            screen.fill(colour as i32 * BAND, 0, BAND, HEIGHT as i32, colour as u8);
        }
        // The band picked, outlined in the lightest and darkest of its sky.
        let x = self.picked as i32 * BAND;
        screen.rect(x - 1, 20, BAND + 2, HEIGHT as i32 - 40, 24);
        screen.text(4, 4, "LEFT RIGHT PICK, A SOUNDS", 24, 1);
        let mut number = [b'0'; 2];
        number[0] += (self.picked / 10) as u8;
        number[1] += (self.picked % 10) as u8;
        let number = core::str::from_utf8(&number).unwrap_or_default();
        screen.text(4, HEIGHT as i32 - 12, number, 24, 2);
    }

    /// The cover's own 32, a dusk from the night sky to the hills, on the
    /// cover and on the bands.
    fn palette(&self) -> Palette {
        COVER.palette()
    }
}

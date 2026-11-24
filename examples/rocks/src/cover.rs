//! Rocks's cover, drawn by the game's own pieces: its ship under thrust,
//! rocks of its three sizes, its name large and the mark at the foot, in
//! its palette. Natively only: `tests/cover.rs` holds `cover.png` to it,
//! and the build script makes that PNG the cover the file carries.

use cartridge::{Palette, Screen};

use crate::draw::{MARK, centred, draw_name};
use crate::{Body, PALETTE, Rocks, SUB, Ship, colour};

/// The seed of its stars and of its rocks' outlines.
const SEED: u32 = 3;

/// Each rock by its size, where it is, in pixels.
const ROCKS: [(u8, i32, i32); 7] = [
    (3, 22, 72),
    (3, 140, 94),
    (2, 132, 60),
    (2, 46, 100),
    (1, 106, 98),
    (1, 16, 48),
    (1, 150, 40),
];

/// Where the ship is, in pixels, and where its nose points.
const SHIP: (i32, i32, u8) = (82, 80, 12);

impl Rocks {
    /// The cover's colours and its pixels.
    #[doc(hidden)]
    pub fn cover() -> (Palette, Screen) {
        let mut game = <Rocks as cartridge::Game>::start(SEED);
        game.clear_field();
        for (size, x, y) in ROCKS {
            game.place(size, Body::at(x * SUB, y * SUB), 0);
        }
        game.ship = Ship {
            body: Body::at(SHIP.0 * SUB, SHIP.1 * SUB),
            angle: SHIP.2,
            alive: true,
            thrusting: true,
            ..Ship::default()
        };
        game.tick = 0;
        let mut screen = Screen::new();
        game.draw_field(&mut screen);
        draw_name(&mut screen, 18);
        centred(&mut screen, 111, MARK, colour::DIM, 1);
        (PALETTE, screen)
    }
}

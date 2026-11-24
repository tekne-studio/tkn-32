//! The frames hash, what two runs of a cartridge compare by: the arcade's
//! workbench and its headless runner print the same one for the same seed
//! and buttons.

use cartridge::{PIXELS, Palette};

use crate::{Cart, Instance};

/// FNV-1a, 64 bits.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

/// The frames hash: FNV-1a, 64 bits, of the screen and the palette after
/// each update run, the one that stopped the cart among them; an update
/// asked of a cart already stopped runs nothing and adds nothing. The same
/// cartridge, seed and buttons give the same hash on every engine, so it
/// says in sixteen digits that two runs showed the same game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frames(u64);

impl Default for Frames {
    fn default() -> Self {
        Frames(FNV_OFFSET)
    }
}

impl Frames {
    /// Adds what `cart` shows after an update.
    pub fn add<I: Instance>(&mut self, cart: &Cart<I>) {
        self.add_shown(cart.screen(), cart.palette());
    }

    pub(crate) fn add_shown(&mut self, screen: &[u8; PIXELS], palette: &Palette) {
        for &byte in screen.iter().chain(palette.as_flattened()) {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
}

impl core::fmt::Display for Frames {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

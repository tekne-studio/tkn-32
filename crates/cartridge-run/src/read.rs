//! A game's screen read whole, as bytes the runner hands a host: its
//! palette, four bytes a colour, then its pixels, an index into it a byte,
//! row by row. What an agent reads of a game is this, never its colours
//! as drawn.

use cartridge::{COLOURS, PIXELS, Palette};

/// How many bytes a screen read is.
pub const READ_BYTES: usize = COLOURS * 4 + PIXELS;

/// A screen and its palette as bytes.
pub fn read_bytes(screen: &[u8; PIXELS], palette: &Palette) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(READ_BYTES);
    bytes.extend(palette.iter().flatten());
    bytes.extend_from_slice(screen);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_reads_as_its_palette_then_its_pixels() {
        let mut palette = [[0; 4]; COLOURS];
        palette[3] = [1, 2, 3, 255];
        let mut screen = [0; PIXELS];
        screen[PIXELS - 1] = 3;
        let bytes = read_bytes(&screen, &palette);
        assert_eq!(bytes.len(), READ_BYTES);
        assert_eq!(bytes[3 * 4..4 * 4], [1, 2, 3, 255]);
        assert_eq!(bytes[COLOURS * 4..], screen);
    }
}

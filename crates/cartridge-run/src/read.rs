//! A game's screen read whole, as bytes across a worker or a thread: its
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

/// The palette and the pixels of bytes [`read_bytes`] made; `None` for
/// bytes of another length.
pub fn read_from_bytes(bytes: &[u8]) -> Option<(Palette, &[u8])> {
    if bytes.len() != READ_BYTES {
        return None;
    }
    let (colours, pixels) = bytes.split_at(COLOURS * 4);
    let mut palette = [[0; 4]; COLOURS];
    for (colour, read) in palette.iter_mut().zip(colours.chunks_exact(4)) {
        colour.copy_from_slice(read);
    }
    Some((palette, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_crosses_as_bytes_and_comes_back_whole() {
        let mut palette = [[0; 4]; COLOURS];
        palette[3] = [1, 2, 3, 255];
        let mut screen = [0; PIXELS];
        screen[PIXELS - 1] = 3;
        let bytes = read_bytes(&screen, &palette);
        let (back, pixels) = read_from_bytes(&bytes).unwrap();
        assert_eq!(back, palette);
        assert_eq!(pixels, screen);
        assert!(read_from_bytes(&bytes[1..]).is_none());
    }
}

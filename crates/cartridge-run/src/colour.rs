//! How a screen is drawn: the palette resolved once, so every host draws a
//! pixel by looking its byte up, and no host holds a rule of its own. A
//! colour's alpha scales its red, green and blue over black, the screen's
//! own dark, and every colour drawn is opaque: a screen looks the same on
//! a page, a texture or a glass whatever lies behind it. A pixel names a
//! colour by its low five bits, so the 32 colours repeat over every value
//! a byte holds.

use cartridge::{COLOURS, PIXELS, Palette};

/// A palette resolved for drawing: the colour each value of a pixel's byte
/// shows, four bytes, opaque.
pub type Resolved = [[u8; 4]; 256];

/// How many bytes [`shown`] writes: the palette resolved, then the pixels.
pub const SHOWN_BYTES: usize = 256 * 4 + PIXELS;

/// `palette` resolved: each colour over black, by its alpha, rounded to
/// the nearest, and opaque; the 32 repeated past the 32nd.
pub fn resolve(palette: &Palette) -> Resolved {
    let mut resolved = [[0; 4]; 256];
    for (to, from) in resolved.iter_mut().zip(palette.iter().cycle()) {
        let alpha = u16::from(from[3]);
        for channel in 0..3 {
            to[channel] = ((u16::from(from[channel]) * alpha + 127) / 255) as u8;
        }
        to[3] = u8::MAX;
    }
    resolved
}

/// What a host draws `screen` from: its palette resolved, then its pixels
/// as the game wrote them, into `into`, [`SHOWN_BYTES`] long.
pub fn shown(screen: &[u8; PIXELS], palette: &Palette, into: &mut [u8]) {
    let (colours, pixels) = into.split_at_mut(256 * 4);
    colours.copy_from_slice(resolve(palette).as_flattened());
    pixels.copy_from_slice(screen);
}

/// The pixels of [`shown`]'s bytes in colour, four bytes each, into
/// `rgba`: each looked up, the whole of a host's drawing.
pub fn draw(shown: &[u8], rgba: &mut [u8]) {
    let (colours, pixels) = shown.split_at(256 * 4);
    for (pixel, &index) in rgba.chunks_exact_mut(4).zip(pixels) {
        let at = usize::from(index) * 4;
        pixel.copy_from_slice(&colours[at..at + 4]);
    }
}

/// `screen` in `palette`'s colours, four bytes a pixel, into `rgba`.
pub(crate) fn rgba(screen: &[u8; PIXELS], palette: &Palette, rgba: &mut [u8]) {
    let resolved = resolve(palette);
    for (pixel, &index) in rgba.chunks_exact_mut(4).zip(screen) {
        pixel.copy_from_slice(&resolved[usize::from(index)]);
    }
}

const _: () = assert!(256 % COLOURS == 0, "the colours repeat whole over a byte");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_shows_over_black_by_its_alpha_and_opaque() {
        let mut palette = [[0; 4]; COLOURS];
        palette[0] = [255, 255, 255, 255];
        palette[1] = [200, 100, 50, 0];
        palette[2] = [200, 100, 51, 128];
        palette[31] = [10, 20, 30, 255];
        let resolved = resolve(&palette);
        assert_eq!(resolved[0], [255, 255, 255, 255]);
        assert_eq!(resolved[1], [0, 0, 0, 255], "no alpha, black");
        assert_eq!(resolved[2], [100, 50, 26, 255], "half, rounded");
        assert_eq!(
            resolved[32 + 2],
            resolved[2],
            "a byte past 31 by its low bits"
        );
        assert_eq!(resolved[255], [10, 20, 30, 255]);
    }

    #[test]
    fn a_host_draws_by_looking_up_what_the_runner_draws() {
        let mut palette = [[0; 4]; COLOURS];
        palette[5] = [90, 180, 240, 64];
        let mut screen = [0; PIXELS];
        screen[7] = 5;
        screen[8] = 5 + 64;
        screen[9] = 255;
        let mut bytes = vec![0; SHOWN_BYTES];
        shown(&screen, &palette, &mut bytes);
        let (mut looked, mut drawn) = (vec![0; PIXELS * 4], vec![0; PIXELS * 4]);
        draw(&bytes, &mut looked);
        rgba(&screen, &palette, &mut drawn);
        assert_eq!(looked, drawn);
        assert_eq!(looked[7 * 4..9 * 4], [23, 45, 60, 255, 23, 45, 60, 255]);
    }
}

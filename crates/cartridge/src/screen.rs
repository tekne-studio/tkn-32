//! The screen a game draws on: one byte a pixel, an index into the palette.
//! Every call clips to the screen, so a shape half off it draws its half.

use crate::font::{self, ADVANCE, GLYPH};
use crate::{HEIGHT, PIXELS, WIDTH};

/// [`WIDTH`] by [`HEIGHT`] pixels, row by row from the top left.
pub struct Screen {
    pixels: [u8; PIXELS],
}

impl Screen {
    /// A screen of colour 0.
    pub const fn new() -> Screen {
        Screen {
            pixels: [0; PIXELS],
        }
    }

    pub fn pixels(&self) -> &[u8; PIXELS] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u8; PIXELS] {
        &mut self.pixels
    }

    /// Paints every pixel `colour`.
    pub fn clear(&mut self, colour: u8) {
        self.pixels.fill(colour);
    }

    /// The colour at `x, y`, or `None` off the screen.
    pub fn get(&self, x: i32, y: i32) -> Option<u8> {
        index(x, y).map(|i| self.pixels[i])
    }

    /// Paints one pixel.
    pub fn pixel(&mut self, x: i32, y: i32, colour: u8) {
        if let Some(i) = index(x, y) {
            self.pixels[i] = colour;
        }
    }

    /// Paints a line from one point to the other, both ends included.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, colour: u8) {
        // In i64: the distance between two i32s overflows an i32.
        let [x0, y0, x1, y1] = [x0, y0, x1, y1].map(i64::from);
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            if let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) {
                self.pixel(x, y, colour);
            }
            if x == x1 && y == y1 {
                return;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Paints the outline of a rectangle `w` by `h` from its top left.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, colour: u8) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.fill(x, y, w, 1, colour);
        self.fill(x, y.saturating_add(h - 1), w, 1, colour);
        self.fill(x, y, 1, h, colour);
        self.fill(x.saturating_add(w - 1), y, 1, h, colour);
    }

    /// Paints a rectangle `w` by `h` from its top left, filled.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, colour: u8) {
        let (x0, x1) = (x.max(0), x.saturating_add(w).min(WIDTH as i32));
        let (y0, y1) = (y.max(0), y.saturating_add(h).min(HEIGHT as i32));
        if x0 >= x1 {
            return;
        }
        for row in y0..y1 {
            let at = row as usize * WIDTH;
            self.pixels[at + x0 as usize..at + x1 as usize].fill(colour);
        }
    }

    /// Writes `text` from its top left, each font pixel `scale` pixels
    /// square; a newline starts a line below. Gives the right edge reached.
    pub fn text(&mut self, x: i32, y: i32, text: &str, colour: u8, scale: i32) -> i32 {
        let (mut pen, mut line, mut right) = (x, y, x);
        for c in text.chars() {
            if c == '\n' {
                (pen, line) = (x, line.saturating_add(ADVANCE[1].saturating_mul(scale)));
                continue;
            }
            if let Some(rows) = font::glyph(c) {
                for (row, bits) in (0..).zip(rows) {
                    for col in 0..GLYPH[0] {
                        if bits & (0b100 >> col) != 0 {
                            let at =
                                |from: i32, by: i32| from.saturating_add(by.saturating_mul(scale));
                            self.fill(at(pen, col), at(line, row), scale, scale, colour);
                        }
                    }
                }
            }
            pen = pen.saturating_add(ADVANCE[0].saturating_mul(scale));
            right = right.max(pen.saturating_sub(scale));
        }
        right
    }

    /// The width `text`'s longest line takes at `scale`.
    pub fn text_width(text: &str, scale: i32) -> i32 {
        let longest = text
            .split('\n')
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) as i32;
        (longest.saturating_mul(ADVANCE[0]) - 1)
            .max(0)
            .saturating_mul(scale)
    }
}

impl Default for Screen {
    fn default() -> Self {
        Screen::new()
    }
}

fn index(x: i32, y: i32) -> Option<usize> {
    ((0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y))
        .then(|| y as usize * WIDTH + x as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_reaches_both_ends_and_clips() {
        let mut screen = Screen::new();
        screen.line(-10, 5, 10, 5, 3);
        assert_eq!(screen.get(0, 5), Some(3));
        assert_eq!(screen.get(10, 5), Some(3));
        assert_eq!(screen.get(11, 5), Some(0));
        screen.line(150, 100, 170, 130, 4);
        assert_eq!(screen.get(150, 100), Some(4));
    }

    #[test]
    fn text_is_three_by_five_with_a_gap() {
        let mut screen = Screen::new();
        let right = screen.text(0, 0, "I1", 7, 1);
        assert_eq!(right, 7);
        assert_eq!(Screen::text_width("I1", 1), 7);
        // I's top row is full, its middle the centre alone.
        assert_eq!([0, 1, 2].map(|x| screen.get(x, 0)), [Some(7); 3]);
        assert_eq!(
            [0, 1, 2].map(|x| screen.get(x, 2)),
            [Some(0), Some(7), Some(0)]
        );
        assert_eq!(screen.get(3, 0), Some(0));
    }

    #[test]
    fn whatever_lies_past_an_edge_is_clipped_never_a_panic() {
        let (w, h) = (WIDTH as i32, HEIGHT as i32);
        let mut screen = Screen::new();
        // Past the right edge on the last row, and far past it on the first.
        screen.fill(w + 1, h - 1, 2, 1, 1);
        screen.fill(30_000, 0, 2, 1, 1);
        // At the ends of an i32, where a sum overflows.
        screen.fill(i32::MAX, i32::MAX, i32::MAX, i32::MAX, 1);
        screen.fill(i32::MIN, i32::MIN, i32::MAX, i32::MAX, 1);
        screen.rect(i32::MAX - 1, 0, i32::MAX, i32::MAX, 1);
        screen.text(i32::MAX - 3, i32::MAX - 3, "W\nW", 1, i32::MAX);
        screen.line(i32::MAX - 2, i32::MIN + 2, i32::MAX, i32::MIN, 1);
        assert_eq!(Screen::text_width("WW", i32::MAX), i32::MAX);
        assert!(screen.pixels().iter().all(|&pixel| pixel == 0));
        // Half on the screen: its half is drawn.
        screen.text(w - 2, h - 5, "AB", 1, 1);
        screen.fill(w - 1, h - 1, 5, 5, 2);
        assert!(screen.pixels().contains(&1));
        assert_eq!(screen.get(w - 1, h - 1), Some(2));
    }

    #[test]
    fn fill_clips_to_the_screen() {
        let mut screen = Screen::new();
        screen.fill(-5, -5, 10, 10, 1);
        screen.fill(WIDTH as i32 - 2, HEIGHT as i32 - 2, 10, 10, 2);
        assert_eq!(screen.get(4, 4), Some(1));
        assert_eq!(screen.get(5, 5), Some(0));
        assert_eq!(screen.get(WIDTH as i32 - 1, HEIGHT as i32 - 1), Some(2));
    }
}

//! A font of 3 by 5 pixels, ASCII from the space to the underscore, and
//! `©`; a small letter is drawn as its capital. Each glyph is five rows of
//! three bits, the high bit on the left. A cover's text is the ASCII alone
//! (`crate::cover::is_text`): `©` is drawn by a game, never kept.

/// A glyph's width and height in pixels.
pub const GLYPH: [i32; 2] = [3, 5];
/// From one glyph to the next, and from one line to the next.
pub const ADVANCE: [i32; 2] = [4, 6];

/// The rows of `c`, or `None` for a character the font does not draw.
pub fn glyph(c: char) -> Option<[u8; 5]> {
    if c == '©' {
        return Some(COPYRIGHT);
    }
    let c = c.to_ascii_uppercase() as u32;
    (32..96).contains(&c).then(|| GLYPHS[(c - 32) as usize])
}

/// `©`, a small c raised to the top of the line, as a mark is printed: a
/// circle around it is more than three pixels hold.
#[rustfmt::skip]
const COPYRIGHT: [u8; 5] = [0b111, 0b100, 0b111, 0b000, 0b000];

#[rustfmt::skip]
const GLYPHS: [[u8; 5]; 64] = [
    [0b000, 0b000, 0b000, 0b000, 0b000], // space
    [0b010, 0b010, 0b010, 0b000, 0b010], // !
    [0b101, 0b101, 0b000, 0b000, 0b000], // "
    [0b101, 0b111, 0b101, 0b111, 0b101], // #
    [0b011, 0b110, 0b010, 0b011, 0b110], // $
    [0b101, 0b001, 0b010, 0b100, 0b101], // %
    [0b010, 0b101, 0b010, 0b101, 0b011], // &
    [0b010, 0b010, 0b000, 0b000, 0b000], // '
    [0b010, 0b100, 0b100, 0b100, 0b010], // (
    [0b010, 0b001, 0b001, 0b001, 0b010], // )
    [0b000, 0b101, 0b010, 0b101, 0b000], // *
    [0b000, 0b010, 0b111, 0b010, 0b000], // +
    [0b000, 0b000, 0b000, 0b010, 0b100], // ,
    [0b000, 0b000, 0b111, 0b000, 0b000], // -
    [0b000, 0b000, 0b000, 0b000, 0b010], // .
    [0b001, 0b001, 0b010, 0b100, 0b100], // /
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b011, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
    [0b000, 0b010, 0b000, 0b010, 0b000], // :
    [0b000, 0b010, 0b000, 0b010, 0b100], // ;
    [0b001, 0b010, 0b100, 0b010, 0b001], // <
    [0b000, 0b111, 0b000, 0b111, 0b000], // =
    [0b100, 0b010, 0b001, 0b010, 0b100], // >
    [0b110, 0b001, 0b010, 0b000, 0b010], // ?
    [0b111, 0b101, 0b111, 0b100, 0b011], // @
    [0b010, 0b101, 0b111, 0b101, 0b101], // A
    [0b110, 0b101, 0b110, 0b101, 0b110], // B
    [0b011, 0b100, 0b100, 0b100, 0b011], // C
    [0b110, 0b101, 0b101, 0b101, 0b110], // D
    [0b111, 0b100, 0b110, 0b100, 0b111], // E
    [0b111, 0b100, 0b110, 0b100, 0b100], // F
    [0b011, 0b100, 0b101, 0b101, 0b011], // G
    [0b101, 0b101, 0b111, 0b101, 0b101], // H
    [0b111, 0b010, 0b010, 0b010, 0b111], // I
    [0b001, 0b001, 0b001, 0b101, 0b010], // J
    [0b101, 0b101, 0b110, 0b101, 0b101], // K
    [0b100, 0b100, 0b100, 0b100, 0b111], // L
    [0b101, 0b111, 0b111, 0b101, 0b101], // M
    [0b110, 0b101, 0b101, 0b101, 0b101], // N
    [0b010, 0b101, 0b101, 0b101, 0b010], // O
    [0b110, 0b101, 0b110, 0b100, 0b100], // P
    [0b010, 0b101, 0b101, 0b110, 0b011], // Q
    [0b110, 0b101, 0b110, 0b101, 0b101], // R
    [0b011, 0b100, 0b010, 0b001, 0b110], // S
    [0b111, 0b010, 0b010, 0b010, 0b010], // T
    [0b101, 0b101, 0b101, 0b101, 0b111], // U
    [0b101, 0b101, 0b101, 0b101, 0b010], // V
    [0b101, 0b101, 0b111, 0b111, 0b101], // W
    [0b101, 0b101, 0b010, 0b101, 0b101], // X
    [0b101, 0b101, 0b010, 0b010, 0b010], // Y
    [0b111, 0b001, 0b010, 0b100, 0b111], // Z
    [0b110, 0b100, 0b100, 0b100, 0b110], // [
    [0b100, 0b100, 0b010, 0b001, 0b001], // \
    [0b011, 0b001, 0b001, 0b001, 0b011], // ]
    [0b010, 0b101, 0b000, 0b000, 0b000], // ^
    [0b000, 0b000, 0b000, 0b000, 0b111], // _
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mark_is_a_glyph_of_its_own() {
        let mark = glyph('©').unwrap();
        assert!(GLYPHS.iter().all(|other| *other != mark));
        assert_eq!(glyph('c'), glyph('C'));
        assert_eq!(glyph('é'), None);
    }
}

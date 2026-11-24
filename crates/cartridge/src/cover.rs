//! A cartridge's cover: its picture, its title, its maker and its year, in
//! a custom section of its file named [`SECTION`]. A host reads it from
//! the file and never runs the game for it; a game may also draw it, as
//! its title screen, from a copy in its data ([`cover!`](crate::cover)).
//! Without the section a file runs as any other, and a host draws its own
//! stand-in.
//!
//! The section, in order, [`MAX_SECTION`] bytes at most:
//!
//! | Field | Bytes | |
//! |---|---|---|
//! | version | 1 | [`VERSION`] |
//! | title | 1 + at most [`MAX_TEXT`] | a length, then the text |
//! | maker | 1 + at most [`MAX_TEXT`] | a length, then the text: `TEKNE STUDIO`, never a `©` or a `by` |
//! | year | 2 | little-endian: a front end composes `© 2026 Tekne Studio` itself |
//! | palette | [`PALETTE_BYTES`] | [`COLOURS`] colours, red, green, blue, alpha |
//! | picture | [`PICTURE_BYTES`] | [`WIDTH`] by [`HEIGHT`], row by row from the top left, packed by [`pack`]: five bits a dot, the highest first |
//!
//! Text is the font's: bytes `0x20` to `0x5F` ([`is_text`]), the capitals,
//! the digits, the punctuation and the apostrophe. A section with a byte
//! outside them, a length past [`MAX_TEXT`], another version, or a size its
//! lengths do not give, is no cover at all ([`Cover::read`]).

use crate::{COLOURS, HEIGHT, PIXELS, Palette, Screen, WIDTH, pack};

/// The custom section's name.
pub const SECTION: &str = "cartridge";
/// The section's layout, its first byte: the one shape, for the day
/// another comes.
pub const VERSION: u8 = 1;
/// The longest title and the longest maker, in bytes.
pub const MAX_TEXT: usize = 24;
/// The palette's bytes.
pub const PALETTE_BYTES: usize = COLOURS * 4;
/// The picture's bytes: eight dots in five.
pub const PICTURE_BYTES: usize = pack::bytes(PIXELS);
/// The bytes of a section with no text: the version, the two lengths, the
/// year, the palette and the picture.
pub const FIXED_BYTES: usize = 1 + 1 + 1 + 2 + PALETTE_BYTES + PICTURE_BYTES;
/// The largest section, both texts at their longest.
pub const MAX_SECTION: usize = FIXED_BYTES + 2 * MAX_TEXT;

/// Whether `byte` is text a cover holds: what the 3 by 5 font draws,
/// `0x20` (the space) to `0x5F` (the underscore).
pub const fn is_text(byte: u8) -> bool {
    matches!(byte, 0x20..=0x5F)
}

/// A cover read from its section's bytes, which it borrows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cover<'a> {
    bytes: &'a [u8],
    title: u8,
    maker: u8,
}

impl<'a> Cover<'a> {
    /// The cover in a section's bytes, or `None` when they are not one:
    /// another version, a length past [`MAX_TEXT`], a byte of text outside
    /// [`is_text`], or a size that is not the one the lengths give.
    /// Constant, so a game's copy is checked as it builds.
    pub const fn read(bytes: &'a [u8]) -> Option<Cover<'a>> {
        if bytes.len() < FIXED_BYTES || bytes[0] != VERSION {
            return None;
        }
        let Some(title) = text_at(bytes, 1) else {
            return None;
        };
        let Some(maker) = text_at(bytes, 2 + title) else {
            return None;
        };
        if bytes.len() != FIXED_BYTES + title + maker {
            return None;
        }
        Some(Cover {
            bytes,
            title: title as u8,
            maker: maker as u8,
        })
    }

    /// Its section's bytes, as read.
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn title(&self) -> &'a str {
        self.text(2, self.title)
    }

    pub fn maker(&self) -> &'a str {
        self.text(3 + usize::from(self.title), self.maker)
    }

    pub fn year(&self) -> u16 {
        let at = self.after_text();
        u16::from_le_bytes([self.bytes[at], self.bytes[at + 1]])
    }

    /// The colours its picture's pixels name.
    pub fn palette(&self) -> Palette {
        let at = self.after_text() + 2;
        let mut palette = [[0; 4]; COLOURS];
        for (colour, bytes) in palette
            .iter_mut()
            .zip(self.bytes[at..at + PALETTE_BYTES].chunks_exact(4))
        {
            colour.copy_from_slice(bytes);
        }
        palette
    }

    /// Its picture as the section holds it, packed ([`pack`]).
    pub fn picture(&self) -> &'a [u8] {
        &self.bytes[self.bytes.len() - PICTURE_BYTES..]
    }

    /// The palette index at `x, y`, or `None` off the picture.
    pub fn pixel(&self, x: usize, y: usize) -> Option<u8> {
        if x >= WIDTH || y >= HEIGHT {
            return None;
        }
        pack::dot(self.picture(), y * WIDTH + x)
    }

    /// Paints the whole screen with its picture, in the indices of its
    /// [`palette`](Cover::palette), which the game gives while it shows it.
    /// Eight pixels from five bytes at a time ([`pack::unpack`]), what a
    /// game's meter charges least.
    pub fn draw(&self, screen: &mut Screen) {
        pack::unpack(self.picture(), screen.pixels_mut());
    }

    fn after_text(&self) -> usize {
        3 + usize::from(self.title) + usize::from(self.maker)
    }

    fn text(&self, at: usize, len: u8) -> &'a str {
        // Read checked every byte is ASCII.
        core::str::from_utf8(&self.bytes[at..at + len as usize]).unwrap_or_default()
    }
}

/// The length of the text whose length byte is at `at`, when it is at most
/// [`MAX_TEXT`], all of it lies within `bytes` and every byte of it is
/// text.
const fn text_at(bytes: &[u8], at: usize) -> Option<usize> {
    if at >= bytes.len() {
        return None;
    }
    let len = bytes[at] as usize;
    if len > MAX_TEXT || at + 1 + len > bytes.len() {
        return None;
    }
    let mut i = at + 1;
    while i < at + 1 + len {
        if !is_text(bytes[i]) {
            return None;
        }
        i += 1;
    }
    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A section by hand: `title`, `maker`, 2026, colour `n` red `n`, and
    /// the picture's pixel `i` index `i % 32`.
    fn section(title: &[u8], maker: &[u8]) -> [u8; MAX_SECTION + 8] {
        let mut out = [0; MAX_SECTION + 8];
        let mut at = 0;
        let mut put = |bytes: &[u8]| {
            out[at..at + bytes.len()].copy_from_slice(bytes);
            at += bytes.len();
        };
        put(&[VERSION, title.len() as u8]);
        put(title);
        put(&[maker.len() as u8]);
        put(maker);
        put(&2026u16.to_le_bytes());
        for n in 0..COLOURS as u8 {
            put(&[n, 0, 0, 255]);
        }
        let dots: [u8; PIXELS] = core::array::from_fn(|i| (i % COLOURS) as u8);
        let mut picture = [0; PICTURE_BYTES];
        pack::pack(&dots, &mut picture);
        put(&picture);
        out
    }

    fn len(title: &[u8], maker: &[u8]) -> usize {
        FIXED_BYTES + title.len() + maker.len()
    }

    #[test]
    fn a_cover_reads_its_title_maker_year_palette_and_picture() {
        let (title, maker) = (b"ROCKS IN SPACE '26", b"TEKNE STUDIO");
        let bytes = section(title, maker);
        let cover = Cover::read(&bytes[..len(title, maker)]).unwrap();
        assert_eq!(cover.title(), "ROCKS IN SPACE '26");
        assert_eq!(cover.maker(), "TEKNE STUDIO");
        assert_eq!(cover.year(), 2026);
        assert_eq!(cover.palette()[5], [5, 0, 0, 255]);
        assert_eq!(cover.pixel(0, 0), Some(0));
        assert_eq!(cover.pixel(1, 0), Some(1));
        assert_eq!(cover.pixel(33, 0), Some(1));
        assert_eq!(cover.pixel(WIDTH, 0), None);
        let mut screen = Screen::new();
        cover.draw(&mut screen);
        assert_eq!(screen.get(33, 0), Some(1));
        assert_eq!(screen.get(31, 119), Some(31));
    }

    #[test]
    fn drawn_every_pixel_is_the_pictures() {
        let (title, maker) = (b"ROCKS", b"");
        let mut bytes = section(title, maker);
        let end = len(title, maker);
        for (i, byte) in bytes[end - PICTURE_BYTES..end].iter_mut().enumerate() {
            *byte = (i as u32).wrapping_mul(2_654_435_761).to_le_bytes()[3];
        }
        let cover = Cover::read(&bytes[..end]).unwrap();
        let mut screen = Screen::new();
        cover.draw(&mut screen);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                assert_eq!(
                    screen.get(x as i32, y as i32),
                    cover.pixel(x, y),
                    "{x}, {y}"
                );
            }
        }
    }

    #[test]
    fn the_largest_section_is_the_sum_of_its_fields() {
        assert_eq!((FIXED_BYTES, MAX_SECTION), (12_133, 12_181));
        let longest = [b'W'; MAX_TEXT];
        let bytes = section(&longest, &longest);
        assert!(Cover::read(&bytes[..MAX_SECTION]).is_some());
    }

    #[test]
    fn another_version_or_size_is_no_cover() {
        let (title, maker) = (b"ROCKS", b"TEKNE");
        let mut bytes = section(title, maker);
        let end = len(title, maker);
        assert!(Cover::read(&bytes[..end]).is_some());
        assert!(Cover::read(&bytes[..end - 1]).is_none());
        assert!(Cover::read(&bytes[..end + 1]).is_none());
        for version in [0, 2, 255] {
            bytes[0] = version;
            assert!(Cover::read(&bytes[..end]).is_none(), "{version}");
        }
    }

    #[test]
    fn text_is_the_fonts_and_nothing_else() {
        for byte in 0..=255u8 {
            let title = [b'A', byte];
            let bytes = section(&title, b"");
            let read = Cover::read(&bytes[..len(&title, b"")]);
            assert_eq!(read.is_some(), (0x20..=0x5f).contains(&byte), "{byte:#x}");
        }
    }

    #[test]
    fn a_cover_is_checked_as_a_game_builds() {
        const BYTES: [u8; MAX_SECTION + 8] = {
            let mut bytes = [0; MAX_SECTION + 8];
            bytes[0] = VERSION;
            bytes
        };
        const READ: Option<Cover<'static>> = Cover::read(BYTES.split_at(FIXED_BYTES).0);
        assert!(READ.is_some_and(|cover| cover.title().is_empty()));
    }
}

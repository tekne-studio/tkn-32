//! A cartridge's cover read from its file, as a host reads it: never by
//! running the game, and from the file as it came, since preparing it drops
//! every custom section. The layout is `cartridge::cover`'s; this walks the
//! file to the one section that holds it.

use cartridge::cover::SECTION;
use cartridge::{Cover, HEIGHT, MAX_FILE, PIXELS, Palette, Screen, WIDTH};
use wasmparser::{Encoding, Parser, Payload};

use crate::cart::{INK, host_screen};
use crate::{Cart, Instance};

/// The cover of `file`, or `None` when it has none: no `cartridge`
/// section, two of them, one that is no cover (`Cover::read`), or a file
/// over `MAX_FILE` or one that does not walk to its end as a WebAssembly
/// module of binary version 1.
pub fn cover(file: &[u8]) -> Option<Cover<'_>> {
    if file.len() > MAX_FILE {
        return None;
    }
    let mut found = None;
    for payload in Parser::new(0).parse_all(file) {
        match payload.ok()? {
            Payload::Version {
                num: 1,
                encoding: Encoding::Module,
                ..
            } => {}
            Payload::Version { .. } => return None,
            Payload::CustomSection(section) if section.name() == SECTION => {
                if found.is_some() {
                    return None;
                }
                found = Some(section.data());
            }
            _ => {}
        }
    }
    Cover::read(found?)
}

/// How many characters of a file's name the stand-in shows: as many as a
/// page shows of a hash.
const NAME_SHOWN: usize = 8;

/// The picture a host shows of a file named `name`, the hash of its bytes,
/// whose cover is `cover` ([`cover`], read once): the cover, or for a file
/// with none, the stand-in, the start of its name on the plain ground a
/// stopped cartridge's screen has, in the same font. Its colours and its
/// pixels.
pub fn cover_screen(cover: Option<&Cover>, name: &str) -> (Palette, Box<Screen>) {
    if let Some(cover) = cover {
        let mut screen = Box::new(Screen::new());
        cover.draw(&mut screen);
        return (cover.palette(), screen);
    }
    let (palette, mut screen) = host_screen();
    let shown: String = name.chars().take(NAME_SHOWN).collect();
    let scale = 2;
    let width = Screen::text_width(&shown, scale);
    let height = cartridge::font::GLYPH[1] * scale;
    screen.text(
        (WIDTH as i32 - width) / 2,
        (HEIGHT as i32 - height) / 2,
        &shown,
        INK,
        scale,
    );
    (palette, screen)
}

/// [`cover_screen`] in colour, four bytes a pixel, `WIDTH` by `HEIGHT`,
/// row by row from the top left, as [`crate::resolve`] draws it: a
/// texture's pixels.
pub fn cover_rgba(cover: Option<&Cover>, name: &str) -> Vec<u8> {
    let (palette, screen) = cover_screen(cover, name);
    let mut rgba = vec![0; PIXELS * 4];
    crate::colour::rgba(screen.pixels(), &palette, &mut rgba);
    rgba
}

/// A cart that shows the picture a host shows of `file`, named `name`
/// ([`cover_screen`]), and never runs: what an idle cabinet's glass shows
/// until someone plays it. Nothing of the file runs, so a game no one
/// plays costs nothing past this.
pub fn still<I: Instance>(file: &[u8], name: &str) -> Cart<I> {
    let (palette, screen) = cover_screen(cover(file).as_ref(), name);
    Cart::showing(palette, screen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_no_cover_shows_the_start_of_its_name() {
        let (palette, screen) = cover_screen(None, "abcdef0123456789");
        assert_eq!(palette[0], host_screen().0[0]);
        let inked = screen
            .pixels()
            .iter()
            .filter(|&&pixel| pixel == INK)
            .count();
        let mut eight = Screen::new();
        eight.text(0, 0, "ABCDEF01", INK, 2);
        assert_eq!(
            inked,
            eight.pixels().iter().filter(|&&pixel| pixel == INK).count()
        );
        assert_eq!(cover_rgba(None, "ab").len(), PIXELS * 4);
    }
}

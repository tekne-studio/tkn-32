//! What a cart's build script calls to give the game its cover, and to
//! link it ([`link`]).
//!
//! The cover: a PNG, 160 by 120 in at most 32 colours, with a title, a
//! maker and a year, checked and written as the bytes of the section
//! `cartridge::cover` lays out. `cartridge::cover!` places those bytes
//! twice in the game: in the custom section a host reads, and in its data,
//! for the game to draw. One picture, so the two cannot drift.
//!
//! ```ignore
//! // build.rs
//! fn main() {
//!     cartridge_build::cover("cover.png", "ROCKS", "TEKNE STUDIO", 2026);
//! }
//! ```
//!
//! The colours are numbered as the painter gives them: a PNG of a palette
//! whose dots all name one of its first 32 keeps each colour at its place
//! in it, so a cover painted in the game's own palette draws in the game's
//! indices; any other PNG numbers its colours in the order they first
//! appear, row by row from the top left.

use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::path::Path;

use cartridge::cover::{MAX_TEXT, PICTURE_BYTES, VERSION, is_text};
use cartridge::{COLOURS, Cover, HEIGHT, PIXELS, WIDTH, pack};

mod link;

pub use link::{MAP, link};

/// The file in the build's `OUT_DIR` that `cartridge::cover!` includes.
pub const FILE: &str = "cover.cartridge";

/// Why a cover cannot be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The PNG would not decode: the decoder's words.
    Png(String),
    /// Its size, other than 160 by 120.
    Size { width: u32, height: u32 },
    /// The colours it holds, more than 32, and the pixel the 33rd first
    /// appears at.
    Colours { count: usize, x: usize, y: usize },
    /// A pixel of a PNG of a palette that names a colour its palette does
    /// not hold, and how many it holds.
    Index {
        x: usize,
        y: usize,
        index: u8,
        colours: usize,
    },
    /// A title or a maker, which is past the longest or holds a character
    /// outside the font's.
    Text { field: &'static str, text: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Png(why) => write!(f, "the cover is no PNG: {why}"),
            Error::Size { width, height } => write!(
                f,
                "the cover is {width} by {height} pixels; it is {WIDTH} by {HEIGHT}"
            ),
            Error::Colours { count, x, y } => write!(
                f,
                "the cover holds {count} colours; it holds {COLOURS} at most, and the \
                 {}rd first appears at {x}, {y}",
                COLOURS + 1
            ),
            Error::Index {
                x,
                y,
                index,
                colours,
            } => write!(
                f,
                "the cover's pixel at {x}, {y} is colour {index}; its palette holds {colours}"
            ),
            Error::Text { field, text } => write!(
                f,
                "the {field} {text:?} is not a cover's: at most {MAX_TEXT} characters of the \
                 font, capitals, digits, the space, punctuation and the apostrophe"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Makes the cover of the cart whose build script calls it: the PNG at
/// `png`, from the cart's folder, checked and written with its words to
/// [`FILE`] in `OUT_DIR`, and the build told to run again when it changes.
/// A cover that cannot be made stops the build, saying why.
pub fn cover(png: &str, title: &str, maker: &str, year: u16) {
    let folder = std::env::var("CARGO_MANIFEST_DIR").expect("run from a build script");
    let out = std::env::var("OUT_DIR").expect("run from a build script");
    let path = Path::new(&folder).join(png);
    println!("cargo::rerun-if-changed={}", path.display());
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("the cover {}: {error}", path.display()));
    let section = section(&bytes, title, maker, year)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    std::fs::write(Path::new(&out).join(FILE), section).expect("OUT_DIR takes the cover");
}

/// The section's bytes of a cover: `png` with its title, its maker and its
/// year.
pub fn section(png: &[u8], title: &str, maker: &str, year: u16) -> Result<Vec<u8>, Error> {
    let (palette, indices) = picture(png)?;
    let mut out = vec![VERSION];
    for (field, text) in [("title", title), ("maker", maker)] {
        if text.len() > MAX_TEXT || !text.bytes().all(is_text) {
            return Err(Error::Text {
                field,
                text: text.to_owned(),
            });
        }
        out.push(text.len() as u8);
        out.extend_from_slice(text.as_bytes());
    }
    out.extend_from_slice(&year.to_le_bytes());
    // The colours no dot names, black.
    let mut padded = [[0, 0, 0, 255]; COLOURS];
    padded[..palette.len()].copy_from_slice(&palette);
    out.extend(padded.iter().flatten());
    let at = out.len();
    out.resize(at + PICTURE_BYTES, 0);
    pack::pack(&indices, &mut out[at..]);
    debug_assert!(Cover::read(&out).is_some());
    Ok(out)
}

/// A PNG's colours, 32 at most, and its pixels as indices into them, one
/// a byte.
fn picture(png: &[u8]) -> Result<(Vec<[u8; 4]>, Vec<u8>), Error> {
    let fail = |error: png::DecodingError| Error::Png(error.to_string());
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(fail)?;
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    if (width, height) != (WIDTH as u32, HEIGHT as u32) {
        return Err(Error::Size { width, height });
    }
    if let (png::ColorType::Indexed, Some(plte)) = (info.color_type, info.palette.as_deref()) {
        // Its own palette, each colour at its place, its alpha from tRNS:
        // as many as its dots can name.
        let colours = (plte.len() / 3).min(COLOURS);
        let trns = info.trns.as_deref().unwrap_or_default();
        let palette: Vec<[u8; 4]> = plte
            .chunks_exact(3)
            .enumerate()
            .map(|(n, rgb)| [rgb[0], rgb[1], rgb[2], trns.get(n).copied().unwrap_or(255)])
            .take(colours)
            .collect();
        let depth = info.bit_depth as usize;
        let mut buffer = vec![0; reader.output_buffer_size().ok_or_else(too_large)?];
        let frame = reader.next_frame(&mut buffer).map_err(fail)?;
        let mut indices = Vec::with_capacity(PIXELS);
        'rows: for (y, row) in buffer
            .chunks_exact(frame.line_size)
            .take(HEIGHT)
            .enumerate()
        {
            for x in 0..WIDTH {
                let bit = x * depth;
                let index = (row[bit / 8] >> (8 - depth - bit % 8)) & ((1 << depth) - 1) as u8;
                // A dot past the 32 cannot keep its place: its colours are
                // numbered as they first appear, below.
                if usize::from(index) >= COLOURS {
                    indices.clear();
                    break 'rows;
                }
                // One past its palette would draw black.
                if usize::from(index) >= colours {
                    return Err(Error::Index {
                        x,
                        y,
                        index,
                        colours,
                    });
                }
                indices.push(index);
            }
        }
        if indices.len() == PIXELS {
            return Ok((palette, indices));
        }
    }
    // Any other: its colours numbered as they first appear.
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(fail)?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or_else(too_large)?];
    let frame = reader.next_frame(&mut buffer).map_err(fail)?;
    let rgba = |pixel: &[u8]| -> [u8; 4] {
        match frame.color_type {
            png::ColorType::Rgba => [pixel[0], pixel[1], pixel[2], pixel[3]],
            png::ColorType::Rgb => [pixel[0], pixel[1], pixel[2], 255],
            png::ColorType::GrayscaleAlpha => [pixel[0], pixel[0], pixel[0], pixel[1]],
            png::ColorType::Grayscale | png::ColorType::Indexed => {
                [pixel[0], pixel[0], pixel[0], 255]
            }
        }
    };
    let channels = frame.color_type.samples();
    let mut numbers: HashMap<[u8; 4], u8> = HashMap::new();
    let mut seen = Vec::new();
    // Where the first colour past the most appears.
    let mut over = None;
    let mut indices = Vec::with_capacity(PIXELS);
    for (y, row) in buffer
        .chunks_exact(frame.line_size)
        .take(HEIGHT)
        .enumerate()
    {
        for (x, pixel) in row.chunks_exact(channels).take(WIDTH).enumerate() {
            let colour = rgba(pixel);
            let n = *numbers.entry(colour).or_insert_with(|| {
                seen.push(colour);
                if seen.len() == COLOURS + 1 {
                    over = Some((x, y));
                }
                (seen.len() - 1) as u8
            });
            indices.push(n);
        }
    }
    if let Some((x, y)) = over {
        return Err(Error::Colours {
            count: seen.len(),
            x,
            y,
        });
    }
    Ok((seen, indices))
}

fn too_large() -> Error {
    Error::Png("too large to decode".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG `width` by `height`, RGBA, each pixel `colour(x, y)`.
    fn rgba_png(width: u32, height: u32, colour: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let data: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| colour(x, y))
            .collect();
        writer.write_image_data(&data).unwrap();
        drop(writer);
        out
    }

    /// A PNG of a palette at `depth` bits, its pixel the index `index(x, y)`.
    fn indexed_png(
        palette: &[[u8; 3]],
        depth: png::BitDepth,
        index: impl Fn(u32) -> u8,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(depth);
        encoder.set_palette(palette.concat());
        let mut writer = encoder.write_header().unwrap();
        let bits = depth as usize;
        let row_bytes = (WIDTH * bits).div_ceil(8);
        let mut data = vec![0; row_bytes * HEIGHT];
        for (y, row) in data.chunks_exact_mut(row_bytes).enumerate() {
            for x in 0..WIDTH {
                let bit = x * bits;
                row[bit / 8] |= index((y * WIDTH + x) as u32) << (8 - bits - bit % 8);
            }
        }
        writer.write_image_data(&data).unwrap();
        drop(writer);
        out
    }

    #[test]
    fn a_cover_is_written_as_the_reader_reads_it() {
        let png = rgba_png(160, 120, |x, _| [(x % 3) as u8 * 100, 0, 0, 255]);
        let section = section(&png, "ROCKS IN SPACE '26", "TEKNE STUDIO", 2026).unwrap();
        let cover = Cover::read(&section).unwrap();
        assert_eq!(cover.title(), "ROCKS IN SPACE '26");
        assert_eq!(cover.maker(), "TEKNE STUDIO");
        assert_eq!(cover.year(), 2026);
        // Numbered as they first appear, the top left first.
        assert_eq!(
            cover.palette()[..3],
            [[0, 0, 0, 255], [100, 0, 0, 255], [200, 0, 0, 255]]
        );
        assert_eq!(
            [0, 1, 2, 3].map(|x| cover.pixel(x, 5)),
            [0, 1, 2, 0].map(Some)
        );
    }

    #[test]
    fn a_palette_keeps_its_places_when_its_dots_name_its_first_32() {
        // 32 in the PNG's palette, the last among them used.
        let palette: Vec<[u8; 3]> = (0..32).map(|n| [n * 8, 0, 1]).collect();
        let png = indexed_png(&palette, png::BitDepth::Eight, |i| (i % 32) as u8);
        let made = section(&png, "A", "B", 1).unwrap();
        let cover = Cover::read(&made).unwrap();
        assert_eq!(cover.palette()[31], [248, 0, 1, 255]);
        assert_eq!(cover.pixel(31, 0), Some(31));
        // A painter's palette of 64, its dots naming the first 20: kept.
        let wide: Vec<[u8; 3]> = (0..64).map(|n| [n * 4, 1, 2]).collect();
        let png = indexed_png(&wide, png::BitDepth::Eight, |i| ((i + 7) % 20) as u8);
        let made = section(&png, "A", "B", 1).unwrap();
        let cover = Cover::read(&made).unwrap();
        assert_eq!(cover.pixel(0, 0), Some(7));
        assert_eq!(cover.palette()[7], [28, 1, 2, 255]);
        // Its dots naming 40 and 50: numbered as they first appear.
        let png = indexed_png(&wide, png::BitDepth::Eight, |i| [40, 50][i as usize % 2]);
        let made = section(&png, "A", "B", 1).unwrap();
        let cover = Cover::read(&made).unwrap();
        assert_eq!([0, 1].map(|x| cover.pixel(x, 0)), [Some(0), Some(1)]);
        assert_eq!(cover.palette()[..2], [[160, 1, 2, 255], [200, 1, 2, 255]]);
    }

    #[test]
    fn a_png_of_a_palette_keeps_its_colours_at_their_places() {
        let palette: Vec<[u8; 3]> = (0..16).map(|n| [n * 16, 255 - n, 7]).collect();
        for depth in [png::BitDepth::Four, png::BitDepth::Eight] {
            // The first pixel is colour 9: never renumbered to 0.
            let png = indexed_png(&palette, depth, |i| ((i + 9) % 16) as u8);
            let section = section(&png, "ROCKS", "TEKNE STUDIO", 2026).unwrap();
            let cover = Cover::read(&section).unwrap();
            assert_eq!(cover.palette()[9], [144, 246, 7, 255], "{depth:?}");
            assert_eq!(cover.pixel(0, 0), Some(9));
            assert_eq!(cover.pixel(159, 119), Some(((19199 + 9) % 16) as u8));
        }
    }

    #[test]
    fn a_pixel_naming_a_colour_its_palette_lacks_is_said() {
        // Four colours, and a pixel of colour 4, or at 8 bits of colour 16:
        // within the 32, past its palette.
        let four: Vec<[u8; 3]> = (0..4).map(|n| [n * 60, 0, 0]).collect();
        for (depth, bad) in [(png::BitDepth::Four, 4), (png::BitDepth::Eight, 16)] {
            let png = indexed_png(&four, depth, |i| if i == 2 * 160 + 7 { bad } else { 3 });
            assert_eq!(
                section(&png, "A", "B", 1),
                Err(Error::Index {
                    x: 7,
                    y: 2,
                    index: bad,
                    colours: 4
                }),
                "{depth:?}"
            );
        }
    }

    #[test]
    fn a_wrong_size_or_too_many_colours_is_said() {
        let small = rgba_png(80, 60, |_, _| [0, 0, 0, 255]);
        assert_eq!(
            section(&small, "A", "B", 1),
            Err(Error::Size {
                width: 80,
                height: 60
            })
        );
        let many = rgba_png(160, 120, |x, _| [x as u8 % 33, 0, 0, 255]);
        let error = section(&many, "A", "B", 1).unwrap_err();
        assert_eq!(
            error,
            Error::Colours {
                count: 33,
                x: 32,
                y: 0
            }
        );
        assert!(error.to_string().contains("33 colours"));
        assert!(error.to_string().contains("33rd first appears at 32, 0"));
        assert!(matches!(
            section(b"no png", "A", "B", 1),
            Err(Error::Png(_))
        ));
    }

    #[test]
    fn words_outside_the_font_or_past_the_longest_are_said() {
        let png = rgba_png(160, 120, |_, _| [0, 0, 0, 255]);
        for (title, maker) in [
            ("Rocks", "TEKNE STUDIO"),
            ("ROCKS", "© TEKNE STUDIO"),
            ("ROCKS\n", "TEKNE"),
            ("A TITLE OF TWENTY FIVE!!", "TEKNE"),
        ] {
            let said = section(&png, title, maker, 2026);
            if title.len() <= MAX_TEXT && title.bytes().all(is_text) && maker.bytes().all(is_text) {
                assert!(said.is_ok(), "{title} {maker}");
            } else {
                assert!(matches!(said, Err(Error::Text { .. })), "{title} {maker}");
            }
        }
        assert!(section(&png, &"W".repeat(25), "TEKNE", 2026).is_err());
        assert!(section(&png, &"W".repeat(24), &"W".repeat(24), 2026).is_ok());
    }
}

//! What a run leaves on disk: pictures of the screen as PNGs, a strip of
//! them all, and the tones heard as a WAV.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use cartridge::{HEIGHT, WIDTH};

/// Saved frames a row in the strip, and its pictures' scale.
const STRIP_COLUMNS: u32 = 4;
const STRIP_SCALE: u32 = 2;
const STRIP_GAP: u32 = 4;
const STRIP_GROUND: [u8; 4] = [40, 40, 40, 255];

/// An RGBA picture.
pub struct Picture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Picture {
    /// The screen in colour, each pixel `scale` pixels wide.
    pub fn of_screen(rgba: &[u8], scale: u32) -> Picture {
        let (width, height) = (WIDTH as u32 * scale, HEIGHT as u32 * scale);
        let mut picture = Picture::filled(width, height, [0; 4]);
        for y in 0..height {
            for x in 0..width {
                let at = ((y / scale) as usize * WIDTH + (x / scale) as usize) * 4;
                picture.put(x, y, &rgba[at..at + 4]);
            }
        }
        picture
    }

    /// The pictures in rows, in order, at the strip's scale; each was made
    /// at `scale`.
    pub fn strip(pictures: &[Picture], scale: u32) -> Picture {
        let (w, h) = (WIDTH as u32 * STRIP_SCALE, HEIGHT as u32 * STRIP_SCALE);
        let columns = STRIP_COLUMNS.min(pictures.len().max(1) as u32);
        let rows = (pictures.len() as u32).div_ceil(columns).max(1);
        let mut strip = Picture::filled(
            columns * (w + STRIP_GAP) - STRIP_GAP,
            rows * (h + STRIP_GAP) - STRIP_GAP,
            STRIP_GROUND,
        );
        for (i, picture) in pictures.iter().enumerate() {
            let (x0, y0) = (
                (i as u32 % columns) * (w + STRIP_GAP),
                (i as u32 / columns) * (h + STRIP_GAP),
            );
            for y in 0..h {
                for x in 0..w {
                    let from = picture.at(x * scale / STRIP_SCALE, y * scale / STRIP_SCALE);
                    strip.put(x0 + x, y0 + y, &from);
                }
            }
        }
        strip
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let said = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
        let file = File::create(path).map_err(|e| said(&e))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| said(&e))?;
        writer.write_image_data(&self.rgba).map_err(|e| said(&e))
    }

    fn filled(width: u32, height: u32, colour: [u8; 4]) -> Picture {
        Picture {
            width,
            height,
            rgba: colour.repeat((width * height) as usize),
        }
    }

    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }

    fn put(&mut self, x: u32, y: u32, colour: &[u8]) {
        let at = ((y * self.width + x) * 4) as usize;
        self.rgba[at..at + 4].copy_from_slice(colour);
    }
}

/// Mono samples as a 16-bit WAV at `rate`.
pub fn save_wav(path: &Path, samples: &[f32], rate: u32) -> Result<(), String> {
    let said = |e: std::io::Error| format!("{}: {e}", path.display());
    let bytes = (samples.len() * 2) as u32;
    let mut wav = BufWriter::new(File::create(path).map_err(said)?);
    let header = [
        b"RIFF".as_slice(),
        &(36 + bytes).to_le_bytes(),
        b"WAVEfmt ",
        &16_u32.to_le_bytes(),
        &1_u16.to_le_bytes(),
        &1_u16.to_le_bytes(),
        &rate.to_le_bytes(),
        &(rate * 2).to_le_bytes(),
        &2_u16.to_le_bytes(),
        &16_u16.to_le_bytes(),
        b"data",
        &bytes.to_le_bytes(),
    ];
    for part in header {
        wav.write_all(part).map_err(said)?;
    }
    for &sample in samples {
        let held = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        wav.write_all(&held.to_le_bytes()).map_err(said)?;
    }
    wav.flush().map_err(said)
}

//! The cover is the game's own drawing: `cover.png` holds exactly what
//! `Rocks::cover` draws, in its palette, colour for colour. Drawn anew,
//! `WRITE_COVER=1 cargo test -p rocks --test cover` writes it.

use cartridge::{Cover, HEIGHT, WIDTH};
use rocks::Rocks;

const PNG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/cover.png");

#[test]
fn the_cover_is_the_games_own_drawing() {
    let (palette, screen) = Rocks::cover();
    if std::env::var_os("WRITE_COVER").is_some() {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Four);
        encoder.set_palette(
            palette
                .iter()
                .flat_map(|c| [c[0], c[1], c[2]])
                .collect::<Vec<_>>(),
        );
        let mut writer = encoder.write_header().unwrap();
        let packed: Vec<u8> = screen
            .pixels()
            .chunks_exact(2)
            .map(|pair| pair[0] << 4 | pair[1])
            .collect();
        writer.write_image_data(&packed).unwrap();
        writer.finish().unwrap();
        std::fs::write(PNG, out).unwrap();
    }
    let section = cartridge_build::section(&std::fs::read(PNG).unwrap(), "", "", 0).unwrap();
    let cover = Cover::read(&section).unwrap();
    assert_eq!(
        cover.palette(),
        palette,
        "cover.png is in the game's palette"
    );
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            assert_eq!(
                cover.pixel(x, y),
                Some(screen.pixels()[y * WIDTH + x]),
                "cover.png at {x}, {y}: WRITE_COVER=1 writes it anew"
            );
        }
    }
}

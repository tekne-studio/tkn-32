//! Covers that lie: a `cartridge` section a host reads from the file before
//! anything runs, and so a way in for bytes nobody checked. Each case reads
//! as no cover at all, never a panic, and leaves the game running exactly
//! as it does without the section: preparing a file drops it.

use std::panic::{AssertUnwindSafe, catch_unwind};

use cartridge::cover::{FIXED_BYTES, MAX_TEXT, SECTION, VERSION};
use cartridge::{COLOURS, MAX_FILE};
use cartridge_run::{cover, prepare};

use crate::support::{assert_runs, cart};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// A cover's section by hand, as `cartridge-build` writes one.
fn section(title: &[u8], maker: &[u8]) -> Vec<u8> {
    let mut out = vec![VERSION, title.len() as u8];
    out.extend_from_slice(title);
    out.push(maker.len() as u8);
    out.extend_from_slice(maker);
    out.extend_from_slice(&2026u16.to_le_bytes());
    for n in 0..COLOURS as u8 {
        out.extend_from_slice(&[n * 8, n, 255 - n, 255]);
    }
    out.extend((0..cartridge::cover::PICTURE_BYTES).map(|i| i as u8));
    out
}

/// `data` as a custom section named `name`.
fn custom(name: &str, data: &[u8]) -> Vec<u8> {
    let mut body = leb(name.len());
    body.extend_from_slice(name.as_bytes());
    body.extend_from_slice(data);
    let mut out = vec![0];
    out.extend(leb(body.len()));
    out.extend(body);
    out
}

fn leb(mut n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

/// A cartridge that keeps the contract, with nothing on its screen.
fn game() -> Vec<u8> {
    cart("", "", 1)
}

/// The game with `sections` after its own.
fn with(sections: &[Vec<u8>]) -> Vec<u8> {
    let mut file = game();
    for section in sections {
        file.extend_from_slice(section);
    }
    file
}

/// Reads `file`'s cover: a panic fails.
fn read(file: &[u8]) -> Option<(String, String, u16)> {
    catch_unwind(AssertUnwindSafe(|| {
        cover(file).map(|cover| {
            (
                cover.title().to_owned(),
                cover.maker().to_owned(),
                cover.year(),
            )
        })
    }))
    .expect("reading a cover panicked")
}

/// Asserts `file` has no cover, and prepares to the same bytes as the game
/// without it: the section reaches no engine.
fn no_cover(file: &[u8], what: &str) {
    assert_eq!(read(file), None, "{what}");
    if let Ok(prepared) = prepare(file) {
        assert_eq!(
            prepared.bytes(),
            prepare(&game()).unwrap().bytes(),
            "{what}: the section reached the prepared file"
        );
    }
}

#[test]
fn a_cover_of_the_fonts_text_reads() {
    let file = with(&[custom(
        SECTION,
        &section(b"ROCKS IN SPACE '26", b"TEKNE STUDIO"),
    )]);
    assert_eq!(
        read(&file),
        Some(("ROCKS IN SPACE '26".into(), "TEKNE STUDIO".into(), 2026))
    );
    // The game runs as it does without one, on every engine.
    assert_eq!(
        prepare(&file).unwrap().bytes(),
        prepare(&game()).unwrap().bytes()
    );
    assert_runs(&file, 3);
}

#[test]
fn a_length_past_the_most_is_no_cover() {
    let long = [b'W'; MAX_TEXT + 1];
    no_cover(
        &with(&[custom(SECTION, &section(&long, b"TEKNE"))]),
        "a title of 25",
    );
    no_cover(
        &with(&[custom(SECTION, &section(b"ROCKS", &long))]),
        "a maker of 25",
    );
    let mut said = section(b"ROCKS", b"");
    said[1] = 255;
    no_cover(&with(&[custom(SECTION, &said)]), "a title length of 255");
}

#[test]
fn a_byte_outside_the_font_is_no_cover() {
    for byte in [
        0x00, b'\n', b'\t', 0x1f, 0x60, b'a', b'~', 0x7f, 0x80, 0xa9, 0xc2, 0xff,
    ] {
        no_cover(
            &with(&[custom(SECTION, &section(&[b'R', byte, b'X'], b"TEKNE"))]),
            &format!("{byte:#04x} in the title"),
        );
        no_cover(
            &with(&[custom(SECTION, &section(b"ROCKS", &[byte]))]),
            &format!("{byte:#04x} in the maker"),
        );
    }
    // "©" in UTF-8 is two bytes, both outside.
    no_cover(
        &with(&[custom(SECTION, &section(b"ROCKS", "© TEKNE".as_bytes()))]),
        "a © in the maker",
    );
}

/// Shorter than its fixed part, a section is refused before any length
/// in it is read: a length can never reach past the end of a section of
/// the fixed size, whose lengths take at most 50 of its 9,669 bytes.
#[test]
fn a_section_shorter_than_its_fixed_part_is_no_cover() {
    // Its title says 20 and the section ends after 5.
    let mut said = vec![VERSION, 20];
    said.extend_from_slice(b"ROCKS");
    no_cover(
        &with(&[custom(SECTION, &said)]),
        "a title running off the end",
    );
    // The maker's length byte is the section's last.
    let mut said = vec![VERSION, 5];
    said.extend_from_slice(b"ROCKS");
    said.push(12);
    no_cover(
        &with(&[custom(SECTION, &said)]),
        "a maker running off the end",
    );
    // A title that takes the whole fixed part, so its maker's length lies
    // past the end.
    let mut said = vec![VERSION, MAX_TEXT as u8];
    said.extend_from_slice(&[b'A'; MAX_TEXT]);
    no_cover(
        &with(&[custom(SECTION, &said)]),
        "no room for the maker's length",
    );
    // The version, then zeros: a byte short of the fixed part. One more,
    // and it is a cover with no words, black.
    let mut zeros = vec![0; FIXED_BYTES];
    zeros[0] = VERSION;
    no_cover(
        &with(&[custom(SECTION, &zeros[..FIXED_BYTES - 1])]),
        "the version and zeros, a byte short",
    );
    assert_eq!(
        read(&with(&[custom(SECTION, &zeros)])),
        Some((String::new(), String::new(), 0))
    );
}

#[test]
fn a_length_that_lands_on_other_bytes_is_no_cover() {
    // The title one shorter: the maker's length is read from the title's
    // last letter, 'S', past the longest.
    let mut said = section(b"ROCKS", b"TEKNE STUDIO");
    said[1] = 4;
    no_cover(&with(&[custom(SECTION, &said)]), "a title said one short");
    // The title one longer: it swallows the maker's length, 12, no text.
    let mut said = section(b"ROCKS", b"TEKNE STUDIO");
    said[1] = 6;
    no_cover(&with(&[custom(SECTION, &said)]), "a title said one long");
}

#[test]
fn a_section_a_byte_short_or_a_byte_long_is_no_cover() {
    let whole = section(b"ROCKS", b"TEKNE STUDIO");
    no_cover(
        &with(&[custom(SECTION, &whole[..whole.len() - 1])]),
        "a byte short",
    );
    let mut long = whole.clone();
    long.push(0);
    no_cover(&with(&[custom(SECTION, &long)]), "a byte long");
    // A trailing byte the font draws, so only the length gives it away.
    let mut trailing = whole.clone();
    trailing.push(b'A');
    no_cover(&with(&[custom(SECTION, &trailing)]), "a trailing letter");
}

#[test]
fn another_version_of_the_layout_is_no_cover() {
    for version in [0, 2, 255] {
        let mut said = section(b"ROCKS", b"TEKNE STUDIO");
        said[0] = version;
        no_cover(
            &with(&[custom(SECTION, &said)]),
            &format!("version {version}"),
        );
    }
}

#[test]
fn two_cartridge_sections_are_no_cover() {
    let one = custom(SECTION, &section(b"ROCKS", b"TEKNE STUDIO"));
    let other = custom(SECTION, &section(b"DOT", b"TEKNE STUDIO"));
    no_cover(&with(&[one.clone(), other]), "two covers");
    no_cover(&with(&[one.clone(), one.clone()]), "the same cover twice");
    // A second one of no cover still spoils the first.
    no_cover(
        &with(&[one, custom(SECTION, &[])]),
        "a cover then an empty one",
    );
}

#[test]
fn an_empty_section_is_no_cover() {
    no_cover(&with(&[custom(SECTION, &[])]), "an empty section");
    no_cover(&with(&[custom(SECTION, &[VERSION])]), "the version alone");
}

#[test]
fn a_section_by_another_name_is_not_the_cover() {
    let said = section(b"ROCKS", b"TEKNE STUDIO");
    for name in ["Cartridge", "cartridge ", "cartridg", "cartridge\0", ""] {
        no_cover(&with(&[custom(name, &said)]), &format!("named {name:?}"));
    }
    // Beside the true one, it changes nothing.
    let file = with(&[custom("cartridge2", &[1, 2, 3]), custom(SECTION, &said)]);
    assert!(read(&file).is_some());
}

#[test]
fn a_cover_in_a_file_that_is_not_webassembly_is_no_cover() {
    let said = custom(SECTION, &section(b"ROCKS", b"TEKNE STUDIO"));
    // Its header alone changed.
    let mut file = with(std::slice::from_ref(&said));
    file[0] = b'X';
    no_cover(&file, "a broken magic number");
    let mut file = with(std::slice::from_ref(&said));
    file[4] = 2;
    no_cover(&file, "another binary version");
    // The cover whole, then a section cut short.
    let mut file = with(std::slice::from_ref(&said));
    file.extend_from_slice(&[1, 40, 0]);
    no_cover(&file, "a section cut after the cover");
    // A section whose size runs past the file, before the cover.
    let mut file = game();
    file.extend_from_slice(&[0, 0xff, 0xff, 0x03]);
    file.extend_from_slice(&said);
    no_cover(&file, "a size that swallows the cover");
    // The cover's own name cut.
    let mut file = game();
    file.extend_from_slice(&said[..4]);
    no_cover(&file, "a cover cut in its name");
    // The cover alone, with no module around it.
    no_cover(&said, "a cover with no header");
    // A component, not a module: its header names another encoding.
    let mut file = b"\0asm\x0d\0\x01\0".to_vec();
    file.extend_from_slice(&said);
    no_cover(&file, "a component");
}

#[test]
fn a_file_of_sixty_thousand_sections_reads_its_cover_or_its_second() {
    let said = custom(SECTION, &section(b"ROCKS", b"TEKNE STUDIO"));
    let many: Vec<u8> = (0..60_000).flat_map(|_| [0, 2, 1, b'x']).collect();
    let mut file = with(std::slice::from_ref(&said));
    file.extend_from_slice(&many);
    assert!(file.len() <= MAX_FILE);
    assert!(read(&file).is_some(), "a cover among them");
    // A second cover at the far end still spoils the first.
    let mut file = with(std::slice::from_ref(&said));
    file.extend_from_slice(&many[..many.len() - said.len()]);
    file.extend_from_slice(&said);
    assert!(file.len() <= MAX_FILE);
    no_cover(&file, "a second cover after them");
}

#[test]
fn a_file_over_the_most_is_no_cover() {
    let said = custom(SECTION, &section(b"ROCKS", b"TEKNE STUDIO"));
    let mut file = with(&[said]);
    // Filled to the most with a section of another name, the cover reads.
    let pad = MAX_FILE - file.len() - 1 - 3 - 1 - 3;
    file.extend(custom("pad", &vec![0; pad]));
    assert_eq!(file.len(), MAX_FILE);
    assert!(read(&file).is_some());
    // One byte more is no cover.
    let mut over = game();
    over.extend(custom(SECTION, &section(b"ROCKS", b"TEKNE STUDIO")));
    over.extend(custom("pad", &vec![0; pad + 1]));
    assert_eq!(over.len(), MAX_FILE + 1);
    assert_eq!(read(&over), None);
}

//! The pads a file says it plays, read by a host before anything runs: its
//! `cartridge.pads` section, one byte, 1 to `PADS`, which the check that
//! judges a file reads on its way. A lie or garbage is refused, on every
//! host; a file that says a count it may, or nothing, runs as the same game
//! without the section, which preparing drops; and a file refused for
//! anything else plays nobody, whatever it says.

use cartridge::MAX_MEMORY;
use cartridge::button::{PADS, PADS_SECTION};
use cartridge_run::{Refusal, prepare};

use crate::support::{assert_runs, cart, custom, refused};

/// A cartridge that keeps the contract, with `sections` after its own.
fn with(sections: &[Vec<u8>]) -> Vec<u8> {
    after(cart("", "", 1), sections)
}

fn after(mut file: Vec<u8>, sections: &[Vec<u8>]) -> Vec<u8> {
    for section in sections {
        file.extend_from_slice(section);
    }
    file
}

/// The pads `file` plays, as a host asks before seating anyone.
fn plays(file: &[u8]) -> Result<u32, Refusal> {
    prepare(file).map(|prepared| prepared.pads())
}

#[test]
fn a_file_says_the_pads_it_plays_and_runs_as_without_it() {
    let game = with(&[]);
    assert_eq!(plays(&game), Ok(1), "a file that says nothing plays one");
    let near = with(&[
        custom("cartridge.pad", &[9]),
        custom("cartridge.pads2", &[9]),
    ]);
    assert_eq!(plays(&near), Ok(1), "another section's name says nothing");
    for pads in 1..=PADS as u8 {
        let file = with(&[custom(PADS_SECTION, &[pads])]);
        assert_eq!(plays(&file), Ok(u32::from(pads)));
        assert_eq!(
            prepare(&file).unwrap().bytes(),
            prepare(&game).unwrap().bytes(),
            "the section reached the prepared file"
        );
        assert_runs(&file, 3);
    }
}

#[test]
fn pads_the_console_has_not_or_garbage_are_refused() {
    let past = PADS as u8 + 1;
    let said = [
        ("no pad", vec![custom(PADS_SECTION, &[0])]),
        (
            "a pad past the console's",
            vec![custom(PADS_SECTION, &[past])],
        ),
        ("255 pads", vec![custom(PADS_SECTION, &[255])]),
        ("no byte", vec![custom(PADS_SECTION, &[])]),
        ("two bytes", vec![custom(PADS_SECTION, &[2, 0])]),
        ("a word", vec![custom(PADS_SECTION, &2u32.to_le_bytes())]),
        (
            "said twice",
            vec![custom(PADS_SECTION, &[2]), custom(PADS_SECTION, &[2])],
        ),
    ];
    for (what, sections) in said {
        let file = with(&sections);
        assert_eq!(plays(&file), Err(Refusal::Pads), "{what}");
        assert_eq!(refused(&file), Refusal::Pads, "{what}");
    }
}

#[test]
fn a_file_refused_for_anything_else_plays_nobody() {
    let pages = (MAX_MEMORY / (64 * 1024)) as u32;
    let file = after(cart("", "", pages + 1), &[custom(PADS_SECTION, &[2])]);
    assert_eq!(plays(&file), Err(Refusal::Memory));
    assert_eq!(refused(&file), Refusal::Memory);
}

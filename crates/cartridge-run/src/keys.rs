//! The keys a keyboard plays the contract's buttons by, by the names a
//! page's keyboard events give them (`KeyboardEvent.code`): one table for
//! every host, so a game plays by the same keys on a page of its own, in
//! the workbench and in a cabinet. A host may add keys of its own, a
//! gamepad's or a touch screen's, and none in the place of these.
//!
//! A keyboard is one person's ([`KEYS`]), every key the first pad's, or two
//! people's ([`SHARED`]), a half each, when a host lets them share it.

use cartridge::button::{A, B, DOWN, LEFT, RIGHT, START, UP, on};

/// Each key and the button it holds down, for one person at the keyboard:
/// the first pad's.
pub const KEYS: [(&str, u32); 15] = [
    ("ArrowLeft", LEFT),
    ("KeyA", LEFT),
    ("ArrowRight", RIGHT),
    ("KeyD", RIGHT),
    ("ArrowUp", UP),
    ("KeyW", UP),
    ("ArrowDown", DOWN),
    ("KeyS", DOWN),
    ("Space", A),
    ("KeyZ", A),
    ("ShiftLeft", B),
    ("ShiftRight", B),
    ("KeyX", B),
    ("Enter", START),
    ("NumpadEnter", START),
];

/// Each key and the button it holds down when two people share one
/// keyboard: the left half the first pad's, its keys of [`KEYS`] under the
/// left hand and 1 for its start; the right half the second pad's, the
/// arrows under the right hand and K and L beside them for the left, with
/// Enter its start. No hand crosses the other person's.
///
/// - The right Shift is no one's: on Windows, while both Shifts are held,
///   letting one go sends no key up, so one person's B would stick on the
///   other's.
/// - The keys a browser takes before a page is asked are left out: `/` and
///   `'` open Firefox's quick find; Tab moves the focus. So are the keys a
///   page keeps for itself: the workbench's `.` steps, `P` pauses, `R`
///   restarts and `M` sounds.
pub const SHARED: [(&str, u32); 17] = [
    ("KeyA", LEFT),
    ("KeyD", RIGHT),
    ("KeyW", UP),
    ("KeyS", DOWN),
    ("Space", A),
    ("KeyZ", A),
    ("ShiftLeft", B),
    ("KeyX", B),
    ("Digit1", START),
    ("ArrowLeft", on(2, LEFT)),
    ("ArrowRight", on(2, RIGHT)),
    ("ArrowUp", on(2, UP)),
    ("ArrowDown", on(2, DOWN)),
    ("KeyK", on(2, A)),
    ("KeyL", on(2, B)),
    ("Enter", on(2, START)),
    ("NumpadEnter", on(2, START)),
];

/// The button `code` holds down for one person at the keyboard; none for a
/// key the table does not name.
pub fn button(code: &str) -> Option<u32> {
    KEYS.iter()
        .find(|(key, _)| *key == code)
        .map(|&(_, button)| button)
}

#[cfg(test)]
mod tests {
    use cartridge::button::{ALL, PADS};

    use super::*;

    /// Whether `table` gives every button of the first `pads` a key, and
    /// no key two buttons.
    fn whole(table: &[(&str, u32)], pads: u32) {
        for pad in 1..=pads {
            for bit in 0..7 {
                let button = on(pad, 1 << bit);
                assert!(
                    table.iter().any(|&(_, held)| held == button),
                    "pad {pad}, button {bit}"
                );
            }
        }
        for (at, (key, _)) in table.iter().enumerate() {
            assert!(
                !table[at + 1..].iter().any(|(other, _)| other == key),
                "{key} twice"
            );
        }
    }

    #[test]
    fn every_button_has_a_key_and_no_key_two_buttons() {
        whole(&KEYS, 1);
        assert_eq!(button("KeyZ"), Some(A));
        assert_eq!(button("KeyQ"), None);
    }

    #[test]
    fn two_people_share_a_keyboard_a_half_each() {
        whole(&SHARED, PADS);
        let first: Vec<_> = SHARED.iter().filter(|(_, held)| held & ALL != 0).collect();
        assert!(
            first
                .iter()
                .all(|(key, _)| KEYS.iter().any(|(own, _)| own == key) || *key == "Digit1"),
            "the first pad keeps its own keys, and a start on its half"
        );
        assert!(!SHARED.iter().any(|(key, _)| *key == "ShiftRight"));
    }
}

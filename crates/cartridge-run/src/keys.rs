//! The keys a keyboard plays the contract's buttons by, by the names a
//! page's keyboard events give them (`KeyboardEvent.code`): one table for
//! every host, so a game plays by the same keys on a page of its own, in
//! the workbench and in a cabinet. A host may add keys of its own, a
//! gamepad's or a touch screen's, and none in the place of these.

use cartridge::button::{A, B, DOWN, LEFT, RIGHT, START, UP};

/// Each key and the button it holds down.
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

/// The button `code` holds down; none for a key the table does not name.
pub fn button(code: &str) -> Option<u32> {
    KEYS.iter()
        .find(|(key, _)| *key == code)
        .map(|&(_, button)| button)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_has_a_key_and_no_key_two_buttons() {
        for bit in 0..7 {
            assert!(
                KEYS.iter().any(|&(_, button)| button == 1 << bit),
                "button {bit}"
            );
        }
        for (at, (key, _)) in KEYS.iter().enumerate() {
            assert!(
                !KEYS[at + 1..].iter().any(|(other, _)| other == key),
                "{key} twice"
            );
        }
        assert_eq!(button("KeyZ"), Some(A));
        assert_eq!(button("KeyQ"), None);
    }
}

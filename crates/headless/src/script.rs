//! A script of buttons: `UPDATE:NAMES` items, held from that update
//! (counted from 0) until the next item's, the names joined by `+` (left
//! right up down a b start) or `-` for none. `#` comments to the line's end.
//! The workbench saves a session in this form (`web/src/lib/session.ts`).
//!
//! ```text
//! 30:start 34:- 60:right+a 100:up # then thrust
//! ```

use cartridge::button;

/// The buttons a script holds, by update.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Script(Vec<(u32, u32)>);

impl Script {
    pub fn parse(script: &str) -> Result<Script, String> {
        let mut items = Vec::new();
        for item in script
            .lines()
            .flat_map(|line| line.split('#').next().unwrap_or("").split([' ', '\t', ',']))
            .filter(|item| !item.is_empty())
        {
            let (from, names) = item
                .split_once(':')
                .ok_or(format!("--buttons: {item:?} wants UPDATE:NAMES"))?;
            let mut held = 0;
            for name in names.split('+').filter(|name| *name != "-") {
                held |= match name {
                    "left" => button::LEFT,
                    "right" => button::RIGHT,
                    "up" => button::UP,
                    "down" => button::DOWN,
                    "a" => button::A,
                    "b" => button::B,
                    "start" => button::START,
                    other => return Err(format!("--buttons: no button {other:?}")),
                };
            }
            let from = from
                .parse()
                .map_err(|_| format!("--buttons: cannot read {from:?} as an update"))?;
            items.push((from, held));
        }
        if !items.is_sorted_by_key(|(from, _)| *from) {
            return Err("--buttons: the updates go in order".into());
        }
        Ok(Script(items))
    }

    /// The buttons held during update `tick`, counted from 0.
    pub fn held(&self, tick: u32) -> u32 {
        let after = self.0.partition_point(|(from, _)| *from <= tick);
        after.checked_sub(1).map_or(0, |at| self.0[at].1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_item_is_held_until_the_next() {
        let script = Script::parse("30:start 34:- # go\n36:left+a").unwrap();
        assert_eq!(script.held(0), 0);
        assert_eq!(script.held(29), 0);
        assert_eq!(script.held(30), button::START);
        assert_eq!(script.held(33), button::START);
        assert_eq!(script.held(34), 0);
        assert_eq!(script.held(36), button::LEFT | button::A);
        assert_eq!(script.held(9_000), button::LEFT | button::A);
    }

    #[test]
    fn what_is_no_script_is_refused() {
        assert!(Script::parse("3:jump").unwrap_err().contains("no button"));
        assert!(Script::parse("5:a 2:b").unwrap_err().contains("in order"));
        assert!(Script::parse("a").unwrap_err().contains("UPDATE:NAMES"));
        assert!(Script::parse("x:a").unwrap_err().contains("as an update"));
    }
}

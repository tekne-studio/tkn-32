//! Cartridges kept running by a clock: each is updated `cartridge::TICKS`
//! times a second from the moment it was put on the shelf, with the
//! buttons its player holds ([`Shelf::press`]); a still ([`crate::still`])
//! shows its cover and never updates. A host runs one shelf off its frame's
//! thread, a page in a worker and a native host on a thread of its own, and
//! reads back each screen that changed and each cart that stopped.
//!
//! A cart that fell behind, its thread busy or asleep, runs at most
//! [`CATCH_UP`] updates in one tick and skips the rest, so it never spends
//! the next frames catching up. What each cart plays is kept an update at
//! a time, an update that played nothing among them, for the host to take
//! ([`Shelf::take_played`]) and sound in the time it was played.

use std::collections::BTreeMap;

use cartridge::TICKS;

use crate::{Cart, Instance, Played};

/// The most updates one cart runs in one tick of the shelf.
pub const CATCH_UP: u64 = 4;

/// Carts by a name the host gives each.
pub struct Shelf<I> {
    carts: BTreeMap<u32, Shelved<I>>,
    /// The updates run since they were last taken, each by its cart.
    played: Vec<Played>,
}

struct Shelved<I> {
    cart: Cart<I>,
    /// When it was put on the shelf, milliseconds on the host's clock.
    began_ms: f64,
    /// The updates its clock has called for so far, run or skipped.
    due: u64,
    /// Its updates and whether it was stopped when its screen was last
    /// said changed; `None` before the first time.
    said: Option<(u64, bool)>,
    /// The buttons held, and those pressed since its last update: a button
    /// pressed and let go between two updates is held for the next one.
    held: u32,
    pressed: u32,
}

impl<I> Default for Shelf<I> {
    fn default() -> Self {
        Shelf {
            carts: BTreeMap::new(),
            played: Vec::new(),
        }
    }
}

impl<I: Instance> Shelf<I> {
    /// Puts `cart` on the shelf as `id`, its clock starting at `now_ms`,
    /// in the place of any cart under that name.
    pub fn put(&mut self, id: u32, cart: Cart<I>, now_ms: f64) {
        let shelved = Shelved {
            cart,
            began_ms: now_ms,
            due: 0,
            said: None,
            held: 0,
            pressed: 0,
        };
        self.carts.insert(id, shelved);
    }

    /// Takes `id` off the shelf, its engine let go.
    pub fn remove(&mut self, id: u32) {
        self.carts.remove(&id);
    }

    pub fn is_empty(&self) -> bool {
        self.carts.is_empty()
    }

    /// Whether a tick would do nothing: no cart on the shelf runs, and
    /// every screen was said since it last changed. A shelf of stills and
    /// stopped carts asks no ticks once each was said.
    pub fn idle(&self) -> bool {
        self.carts.values().all(|shelved| {
            !shelved.cart.runs()
                && shelved.said == Some((shelved.cart.updates(), shelved.cart.stop().is_some()))
        })
    }

    /// Holds `buttons` down on `id`, in the place of what was held, from
    /// its next update on.
    pub fn press(&mut self, id: u32, buttons: u32) {
        if let Some(shelved) = self.carts.get_mut(&id) {
            shelved.held = buttons;
            shelved.pressed |= buttons;
        }
    }

    /// Whether `id` stopped for good: its screen says why.
    pub fn stopped(&self, id: u32) -> bool {
        self.carts
            .get(&id)
            .is_some_and(|shelved| shelved.cart.stop().is_some())
    }

    /// Runs every cart's updates due by `now_ms`, with the buttons held on
    /// it, and says which carts' screens changed since last said, a cart
    /// put on the shelf or stopped among them. What each update played is
    /// kept, in the order run, until taken.
    pub fn tick(&mut self, now_ms: f64) -> Vec<u32> {
        let mut changed = Vec::new();
        for (&id, shelved) in &mut self.carts {
            let elapsed_ms = (now_ms - shelved.began_ms).max(0.0);
            let due = (elapsed_ms * f64::from(TICKS) / 1000.0) as u64;
            let behind = due.saturating_sub(shelved.due);
            for _ in 0..behind.min(CATCH_UP) {
                // A still, or a cart stopped, runs no update and plays none.
                if !shelved.cart.runs() {
                    break;
                }
                let tones = shelved.cart.update(shelved.held | shelved.pressed);
                self.played.push(Played {
                    cart: id,
                    tones: tones.to_vec(),
                });
                shelved.pressed = 0;
            }
            shelved.due = shelved.due.max(due);
            let now = (shelved.cart.updates(), shelved.cart.stop().is_some());
            if shelved.said != Some(now) {
                shelved.said = Some(now);
                changed.push(id);
            }
        }
        changed
    }

    /// The updates the carts ran since last taken, each with its cart's
    /// name and what it played, in the order run: a cart taken off the
    /// shelf keeps what it played before.
    pub fn take_played(&mut self) -> Vec<Played> {
        core::mem::take(&mut self.played)
    }

    /// The screen of `id` and its palette, as [`crate::read_bytes`] makes
    /// them; `None` when no cart is there by that name.
    pub fn read(&self, id: u32) -> Option<Vec<u8>> {
        let shelved = self.carts.get(&id)?;
        Some(crate::read_bytes(
            shelved.cart.screen(),
            shelved.cart.palette(),
        ))
    }

    /// The screen of `id` in colour, four bytes a pixel, into `into`;
    /// false when no cart is there by that name.
    pub fn rgba(&self, id: u32, into: &mut [u8]) -> bool {
        let Some(shelved) = self.carts.get(&id) else {
            return false;
        };
        shelved.cart.rgba(into);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use cartridge::{Export, PIXELS, UPDATE_BUDGET};

    use super::*;
    use crate::{Played, Stop, Tone, Trapped};

    /// An engine that counts its updates, keeps the buttons each was
    /// given, and traps on the one it is told.
    struct Counting {
        updates: u32,
        trap_at: u32,
        buttons: Rc<RefCell<Vec<u32>>>,
    }

    impl Instance for Counting {
        fn start(&mut self) -> Result<(), Trapped> {
            Ok(())
        }

        fn call(&mut self, export: Export, argument: u32) -> Result<u32, Trapped> {
            match export {
                Export::CartAbi => Ok(cartridge::ABI),
                Export::Update => {
                    self.updates += 1;
                    self.buttons.borrow_mut().push(argument);
                    match self.updates == self.trap_at {
                        true => Err(Trapped),
                        false => Ok(0),
                    }
                }
                Export::Frame => Ok(0),
                Export::Palette => Ok(PIXELS as u32),
                Export::Init | Export::Screen => Ok(0),
            }
        }

        fn read(&self, _: u32, into: &mut [u8]) -> bool {
            into.fill(0);
            true
        }

        fn meter(&self) -> i32 {
            UPDATE_BUDGET as i32
        }

        fn set_meter(&mut self, _: i32) {}

        /// A tone each update, a step higher each, and none every third.
        fn take_tones(&mut self) -> Vec<Tone> {
            match self.updates % 3 {
                0 => vec![],
                _ => vec![Tone::steady(1, 400 + self.updates, 50, 0)],
            }
        }
    }

    fn cart(trap_at: u32) -> Cart<Counting> {
        pressed(trap_at).0
    }

    /// A cart, and the buttons each of its updates was given.
    fn pressed(trap_at: u32) -> (Cart<Counting>, Rc<RefCell<Vec<u32>>>) {
        let buttons = Rc::default();
        let counting = Counting {
            updates: 0,
            trap_at,
            buttons: Rc::clone(&buttons),
        };
        (Cart::start(counting, 0), buttons)
    }

    const TICK_MS: f64 = 1000.0 / TICKS as f64;

    #[test]
    fn a_cart_runs_by_its_clock_and_is_said_when_its_screen_changes() {
        let mut shelf = Shelf::default();
        shelf.put(1, cart(0), 1000.0);
        assert_eq!(shelf.tick(1000.0), [1], "a cart put on the shelf is said");
        assert!(
            shelf.tick(1000.0 + TICK_MS / 2.0).is_empty(),
            "no update due"
        );
        assert_eq!(shelf.tick(1000.0 + TICK_MS * 2.5), [1]);
        assert_eq!(shelf.carts[&1].cart.updates(), 2);
        let mut rgba = vec![0; PIXELS * 4];
        assert!(shelf.rgba(1, &mut rgba));
        assert!(!shelf.rgba(2, &mut rgba));
    }

    #[test]
    fn a_still_shows_its_cover_once_and_never_runs_stops_or_plays() {
        let mut shelf = Shelf::<Counting>::default();
        shelf.put(1, crate::still(b"", "c0ffee"), 0.0);
        assert_eq!(shelf.tick(0.0), [1], "said once, as it is put");
        assert!(shelf.tick(TICK_MS * 30.5).is_empty());
        assert!(!shelf.stopped(1));
        assert!(shelf.take_played().is_empty());
        let mut rgba = vec![0; PIXELS * 4];
        assert!(shelf.rgba(1, &mut rgba));
        assert_eq!(rgba, crate::cover_rgba(None, "c0ffee"));
    }

    #[test]
    fn a_shelf_of_stills_and_stopped_carts_is_idle_once_each_was_said() {
        let mut shelf = Shelf::default();
        assert!(shelf.idle(), "an empty shelf");
        shelf.put(1, crate::still(b"", "c0ffee"), 0.0);
        assert!(!shelf.idle(), "a still not yet said");
        shelf.tick(0.0);
        assert!(shelf.idle());
        shelf.put(2, cart(2), 0.0);
        shelf.tick(0.0);
        assert!(!shelf.idle(), "a cart that runs");
        shelf.tick(TICK_MS * 2.5);
        assert!(shelf.stopped(2));
        assert!(shelf.idle(), "stopped, its stop screen said");
        assert!(shelf.tick(TICK_MS * 60.5).is_empty());
    }

    #[test]
    fn a_press_between_two_updates_is_held_for_the_next() {
        let mut shelf = Shelf::default();
        let (cart, buttons) = pressed(0);
        shelf.put(1, cart, 0.0);
        shelf.press(1, 0b1);
        shelf.press(1, 0);
        shelf.tick(TICK_MS * 1.5);
        assert_eq!(*buttons.borrow(), [0b1]);
        shelf.press(1, 0b10);
        shelf.tick(TICK_MS * 3.5);
        assert_eq!(*buttons.borrow(), [0b1, 0b10, 0b10]);
        assert!(!shelf.stopped(1));
    }

    #[test]
    fn each_update_a_cart_runs_is_kept_by_its_name_until_taken() {
        let mut shelf = Shelf::default();
        shelf.put(1, cart(0), 0.0);
        shelf.put(2, cart(0), 0.0);
        shelf.tick(TICK_MS * 3.5);
        let hz = |played: Vec<Played>| -> Vec<(u32, Vec<u32>)> {
            let hz = |tones: Vec<Tone>| tones.into_iter().map(|tone| tone.hz).collect();
            played
                .into_iter()
                .map(|update| (update.cart, hz(update.tones)))
                .collect()
        };
        assert_eq!(
            hz(shelf.take_played()),
            [
                (1, vec![401]),
                (1, vec![402]),
                (1, vec![]),
                (2, vec![401]),
                (2, vec![402]),
                (2, vec![])
            ],
            "an update that played nothing is kept in its place"
        );
        assert!(shelf.take_played().is_empty(), "taken once");
        shelf.tick(TICK_MS * 4.5);
        shelf.remove(2);
        assert_eq!(hz(shelf.take_played()), [(1, vec![404]), (2, vec![404])]);
    }

    #[test]
    fn a_cart_behind_catches_up_a_few_updates_and_skips_the_rest() {
        let mut shelf = Shelf::default();
        shelf.put(1, cart(0), 0.0);
        shelf.tick(TICK_MS * 100.5);
        assert_eq!(shelf.carts[&1].cart.updates(), CATCH_UP);
        shelf.tick(TICK_MS * 101.5);
        assert_eq!(shelf.carts[&1].cart.updates(), CATCH_UP + 1);
    }

    #[test]
    fn a_cart_whose_file_never_came_is_said_once_as_stopped() {
        let mut shelf = Shelf::<Counting>::default();
        shelf.put(1, Cart::stopped(Stop::Missing), 0.0);
        assert_eq!(shelf.tick(0.0), [1], "its stop screen is said");
        assert!(shelf.stopped(1));
        assert!(shelf.tick(TICK_MS * 9.5).is_empty(), "and never again");
        assert_eq!(Stop::Missing.to_string(), "its file did not come");
    }

    #[test]
    fn a_cart_that_stops_is_said_once_and_its_neighbours_run_on() {
        let mut shelf = Shelf::default();
        shelf.put(1, cart(2), 0.0);
        shelf.put(2, cart(0), 0.0);
        shelf.tick(0.0);
        assert_eq!(shelf.tick(TICK_MS * 3.5), [1, 2]);
        assert_eq!(shelf.carts[&1].cart.stop(), Some(&Stop::Trap));
        assert!(shelf.stopped(1) && !shelf.stopped(2));
        assert_eq!(
            shelf.tick(TICK_MS * 4.5),
            [2],
            "a stopped screen changes no more"
        );
        shelf.remove(2);
        assert!(shelf.tick(TICK_MS * 9.5).is_empty());
        assert!(!shelf.is_empty());
    }
}

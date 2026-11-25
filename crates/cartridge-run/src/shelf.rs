//! Cartridges kept running by a clock: each is updated `cartridge::TICKS`
//! times a second from the moment it was put on the shelf, times its pace
//! ([`Shelf::pace`], 0 holding it), with the buttons its player holds
//! ([`Shelf::press`]) or those of a script ([`Shelf::script`]); a host may
//! also run a cart's next updates at once, outside its clock
//! ([`Shelf::step`]). A still ([`crate::still`]) shows its cover and never
//! updates. A host runs one shelf off its frame's thread, a page in a
//! worker and a native host on a thread of its own, and reads back each
//! screen that changed and each cart that stopped.
//!
//! A cart that fell behind, its thread busy or asleep, runs at most
//! [`CATCH_UP`] updates a tick at its pace and skips the rest, so it never
//! spends the next frames catching up. What each cart plays is kept an
//! update at a time, an update that played nothing among them, for the host
//! to take ([`Shelf::sound`]) and sound in the time it was played.

use std::collections::{BTreeMap, VecDeque};

use cartridge::{TICKS, button};

use crate::{Cart, Instance, Tone, tones::batch};

/// The most updates one cart runs in one tick of the shelf, at pace 1.
pub const CATCH_UP: u64 = 4;
/// The fastest a cart's clock runs: four times the contract's pace.
pub const FASTEST: f64 = 4.0;
/// The most updates whose tones a cart keeps for the host to take: ten
/// seconds of a game at the contract's pace. A host takes them after each
/// tick or step; one that never does keeps only the last.
pub const KEPT: usize = 600;
/// The most updates one script holds, the longest session the console
/// keeps: about 19 hours at the contract's pace. A script is a session
/// read back, and a file is hostile input: a longer one would only fill
/// the memory of whatever holds it.
pub const LONGEST: usize = 1 << 22;

/// Carts by a name the host gives each.
pub struct Shelf<I> {
    carts: BTreeMap<u32, Shelved<I>>,
}

struct Shelved<I> {
    cart: Cart<I>,
    clock: Clock,
    /// Its updates and whether it was stopped when its screen was last
    /// said changed; `None` before the first time.
    said: Option<(u64, bool)>,
    /// The buttons held, and those pressed since its last update: a button
    /// pressed and let go between two updates is held for the next one.
    held: u32,
    pressed: u32,
    /// The buttons of the updates to come, one an update, played in the
    /// place of the held ones until they run out or a button goes down;
    /// never empty.
    script: Option<VecDeque<u32>>,
    /// What each update played, oldest first, until taken.
    played: VecDeque<Vec<Tone>>,
}

/// A cart's clock: how many updates it has called for by a time on the
/// host's clock, at a pace that changes.
struct Clock {
    /// When the pace was last set, and how many updates were due then, a
    /// part of the next among them.
    since_ms: f64,
    due_since: f64,
    pace: f64,
    /// The updates it has called for so far, run or skipped.
    called: u64,
}

impl Clock {
    fn due(&self, now_ms: f64) -> f64 {
        let elapsed_ms = (now_ms - self.since_ms).max(0.0);
        self.due_since + elapsed_ms * f64::from(TICKS) / 1000.0 * self.pace
    }

    fn set(&mut self, pace: f64, now_ms: f64) {
        if pace != self.pace {
            self.due_since = self.due(now_ms);
            self.since_ms = now_ms;
            self.pace = pace;
        }
    }

    /// The most updates one tick runs, as many more as it runs faster.
    fn most(&self) -> u64 {
        (CATCH_UP as f64 * self.pace.max(1.0)).ceil() as u64
    }
}

impl<I: Instance> Shelved<I> {
    /// The buttons of the next update, the script's next or those held
    /// and pressed, and whether the script runs out with it.
    fn next(&mut self) -> (u32, bool) {
        let Some(script) = &mut self.script else {
            return (self.held | core::mem::take(&mut self.pressed), false);
        };
        let buttons = script.pop_front().unwrap_or(0);
        let out = script.is_empty();
        if out {
            self.script = None;
        }
        (buttons, out)
    }

    fn run(&mut self, buttons: u32) {
        let tones = self.cart.update(buttons).to_vec();
        if self.played.len() == KEPT {
            self.played.pop_front();
        }
        self.played.push_back(tones);
    }
}

impl<I> Default for Shelf<I> {
    fn default() -> Self {
        Shelf {
            carts: BTreeMap::new(),
        }
    }
}

impl<I: Instance> Shelf<I> {
    /// Puts `cart` on the shelf as `id`, its clock starting at `now_ms` at
    /// the contract's pace, in the place of any cart under that name.
    pub fn put(&mut self, id: u32, cart: Cart<I>, now_ms: f64) {
        let shelved = Shelved {
            cart,
            clock: Clock {
                since_ms: now_ms,
                due_since: 0.0,
                pace: 1.0,
                called: 0,
            },
            said: None,
            held: 0,
            pressed: 0,
            script: None,
            played: VecDeque::new(),
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

    /// Whether a tick would do nothing: no cart on the shelf runs by its
    /// clock, and every screen was said since it last changed. A shelf of
    /// stills, stopped carts and held ones asks no ticks once each was said.
    pub fn idle(&self) -> bool {
        self.carts.values().all(|shelved| {
            let cart = &shelved.cart;
            (!cart.runs() || shelved.clock.pace == 0.0)
                && shelved.said == Some((cart.updates(), cart.stop().is_some()))
        })
    }

    /// Holds `buttons` down on `id`, in the place of what was held, from
    /// its next update on. A button that goes down ends its script; a bit
    /// no pad has is no button, and ends none.
    pub fn press(&mut self, id: u32, buttons: u32) {
        let buttons = buttons & button::EVERY;
        if let Some(shelved) = self.carts.get_mut(&id) {
            if buttons & !shelved.held != 0 {
                shelved.script = None;
            }
            shelved.held = buttons;
            shelved.pressed |= buttons;
        }
    }

    /// Runs `id`'s clock at `pace` times the contract's from `now_ms`, up to
    /// [`FASTEST`]; 0, or a pace that is no number, holds it.
    pub fn pace(&mut self, id: u32, pace: f64, now_ms: f64) {
        if let Some(shelved) = self.carts.get_mut(&id) {
            let pace = if pace > 0.0 { pace.min(FASTEST) } else { 0.0 };
            shelved.clock.set(pace, now_ms);
        }
    }

    /// Plays `buttons` on `id`, one an update, after any script it plays,
    /// in the place of the buttons held: its clock holds the cart at the
    /// script's last update, and a button that goes down ends it. A script
    /// that starts is the buttons as given, a `Vec`'s taken without a copy.
    pub fn script(&mut self, id: u32, buttons: impl Into<VecDeque<u32>>) {
        let mut buttons = buttons.into();
        let Some(shelved) = self.carts.get_mut(&id) else {
            return;
        };
        match &mut shelved.script {
            _ if buttons.is_empty() => {}
            Some(script) => script.append(&mut buttons),
            None => {
                shelved.pressed = 0;
                shelved.script = Some(buttons);
            }
        }
    }

    /// How many updates of `id`'s script are left to play; `None` when it
    /// plays none.
    pub fn scripted(&self, id: u32) -> Option<usize> {
        self.carts.get(&id)?.script.as_ref().map(VecDeque::len)
    }

    /// Runs `id`'s next `count` updates now, outside its clock, with its
    /// script's buttons and then the held ones; fewer when it stops, and
    /// [`KEPT`] at most: past them the first updates' tones are gone before
    /// a host takes them, and one call never holds a host for long. Gives
    /// how many ran.
    pub fn step(&mut self, id: u32, count: u32) -> u32 {
        let Some(shelved) = self.carts.get_mut(&id) else {
            return 0;
        };
        let count = count.min(KEPT as u32);
        let mut ran = 0;
        while ran < count && shelved.cart.runs() {
            let (buttons, _) = shelved.next();
            shelved.run(buttons);
            ran += 1;
        }
        if ran > 0 {
            shelved.cart.look();
        }
        ran
    }

    /// Whether `id` stopped for good: its screen says why.
    pub fn stopped(&self, id: u32) -> bool {
        self.carts
            .get(&id)
            .is_some_and(|shelved| shelved.cart.stop().is_some())
    }

    /// Runs every cart's updates due by `now_ms`, and says which carts'
    /// screens changed since last said, a cart put on the shelf or stopped
    /// among them. What each update played is kept, in the order run,
    /// until taken.
    pub fn tick(&mut self, now_ms: f64) -> Vec<u32> {
        let mut changed = Vec::new();
        for (&id, shelved) in &mut self.carts {
            let due = shelved.clock.due(now_ms) as u64;
            let behind = due.saturating_sub(shelved.clock.called);
            let mut ran = 0;
            // A still, or a cart stopped, runs no update and plays none.
            while ran < behind.min(shelved.clock.most()) && shelved.cart.runs() {
                let (buttons, out) = shelved.next();
                shelved.run(buttons);
                ran += 1;
                if out {
                    shelved.clock.set(0.0, now_ms);
                    break;
                }
            }
            shelved.clock.called = shelved.clock.called.max(due);
            if ran > 0 {
                shelved.cart.look();
            }
            let now = (shelved.cart.updates(), shelved.cart.stop().is_some());
            if shelved.said != Some(now) {
                shelved.said = Some(now);
                changed.push(id);
            }
        }
        changed
    }

    /// The cart on the shelf as `id`: its screen, its updates, why it
    /// stopped, what it shows of itself when watched.
    pub fn cart(&self, id: u32) -> Option<&Cart<I>> {
        Some(&self.carts.get(&id)?.cart)
    }

    /// What `id` played in each update since last taken, oldest first, an
    /// update that played nothing among them.
    pub fn take_played(&mut self, id: u32) -> Vec<Vec<Tone>> {
        match self.carts.get_mut(&id) {
            Some(shelved) => shelved.played.drain(..).collect(),
            None => Vec::new(),
        }
    }

    /// What `id` played since last taken, as the arcade's chip takes it
    /// ([`batch`]): laid an update apart at its pace, or all at once while
    /// its clock is held. Empty when it ran no update.
    pub fn sound(&mut self, id: u32) -> Vec<u32> {
        let Some(shelved) = self.carts.get_mut(&id) else {
            return Vec::new();
        };
        if shelved.played.is_empty() {
            return Vec::new();
        }
        let per_second = (f64::from(TICKS) * shelved.clock.pace).round() as u32;
        let sound = batch(per_second, shelved.played.iter().map(Vec::as_slice));
        shelved.played.clear();
        sound
    }

    /// The screen of `id` and its palette, as [`crate::read_bytes`] makes
    /// them; `None` when no cart is there by that name.
    pub fn read(&self, id: u32) -> Option<Vec<u8>> {
        let cart = self.cart(id)?;
        Some(crate::read_bytes(cart.screen(), cart.palette()))
    }

    /// The screen of `id` in colour, four bytes a pixel, into `into`;
    /// false when no cart is there by that name.
    pub fn rgba(&self, id: u32, into: &mut [u8]) -> bool {
        self.cart(id).map(|cart| cart.rgba(into)).is_some()
    }

    /// What a host draws `id`'s screen from ([`crate::shown`]), into
    /// `into`; false when no cart is there by that name.
    pub fn shown(&self, id: u32, into: &mut [u8]) -> bool {
        self.cart(id).map(|cart| cart.shown(into)).is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use cartridge::{Export, PIXELS, UPDATE_BUDGET};

    use super::*;
    use crate::{BLOCK, Frames, Ran, Stop, Tone, Trapped};

    /// An engine that counts its updates, keeps the buttons each was
    /// given, and traps on the one it is told. Its memory is a page of
    /// zeros but for the count of its updates, at 1024.
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

        fn read(&self, at: u32, into: &mut [u8]) -> bool {
            if at as usize + into.len() > PAGE {
                return false;
            }
            into.fill(0);
            if let Some(count) = 1024usize
                .checked_sub(at as usize)
                .and_then(|i| into.get_mut(i))
            {
                *count = self.updates as u8;
            }
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
    const PAGE: usize = 64 * 1024;

    /// The buttons a cart was given, by update.
    fn given(buttons: &Rc<RefCell<Vec<u32>>>) -> Vec<u32> {
        buttons.borrow().clone()
    }

    /// Ticks once a frame, a half frame past each, from frame `from` to
    /// frame `to`.
    fn frames<I: Instance>(shelf: &mut Shelf<I>, from: u32, to: u32) {
        for frame in from..=to {
            shelf.tick((f64::from(frame) + 0.5) * TICK_MS);
        }
    }

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
        assert!(shelf.take_played(1).is_empty());
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
    fn each_update_a_cart_runs_is_kept_for_its_cart_until_taken() {
        let mut shelf = Shelf::default();
        shelf.put(1, cart(0), 0.0);
        shelf.put(2, cart(0), 0.0);
        shelf.tick(TICK_MS * 3.5);
        let hz = |played: Vec<Vec<Tone>>| -> Vec<Vec<u32>> {
            let hz = |tones: Vec<Tone>| tones.into_iter().map(|tone| tone.hz).collect();
            played.into_iter().map(hz).collect()
        };
        let three = [vec![401], vec![402], vec![]];
        assert_eq!(
            hz(shelf.take_played(1)),
            three,
            "an update that played nothing is kept in its place"
        );
        assert!(shelf.take_played(1).is_empty(), "taken once");
        shelf.tick(TICK_MS * 4.5);
        assert_eq!(
            hz(shelf.take_played(2)),
            [&three[..], &[vec![404]]].concat()
        );
        shelf.remove(2);
        assert!(shelf.take_played(2).is_empty(), "gone with its cart");
    }

    #[test]
    fn what_a_cart_played_is_taken_as_the_chips_batch_at_its_pace() {
        let mut shelf = Shelf::default();
        shelf.put(1, cart(0), 0.0);
        assert!(shelf.sound(1).is_empty(), "no update ran");
        shelf.tick(TICK_MS * 2.5);
        let (first, second) = (Tone::steady(1, 401, 50, 0), Tone::steady(1, 402, 50, 0));
        let at_pace = batch(TICKS, [&[first][..], &[second]]);
        assert_eq!(shelf.sound(1), at_pace);
        assert!(shelf.sound(1).is_empty(), "taken once");
        shelf.pace(1, 0.5, TICK_MS * 2.5);
        frames(&mut shelf, 3, 4);
        assert_eq!(shelf.sound(1), batch(TICKS / 2, [&[][..]]));
        shelf.pace(1, 0.0, TICK_MS * 4.5);
        shelf.step(1, 1);
        let held = Tone::steady(1, 404, 50, 0);
        assert_eq!(shelf.sound(1), batch(0, [&[held][..]]), "all at once");
    }

    #[test]
    fn a_cart_runs_at_its_pace_and_holds_at_none() {
        let mut shelf = Shelf::default();
        shelf.put(1, cart(0), 0.0);
        let updates = |shelf: &Shelf<Counting>| shelf.cart(1).unwrap().updates();
        shelf.pace(1, 0.5, 0.0);
        frames(&mut shelf, 0, 20);
        assert_eq!(updates(&shelf), 10, "half the contract's pace");
        shelf.pace(1, 0.0, TICK_MS * 20.5);
        frames(&mut shelf, 21, 30);
        assert_eq!(updates(&shelf), 10, "held");
        assert!(shelf.idle());
        shelf.pace(1, 2.0, TICK_MS * 30.5);
        frames(&mut shelf, 31, 40);
        assert_eq!(updates(&shelf), 30, "twice, from where it was held");
        shelf.pace(1, f64::NAN, TICK_MS * 40.5);
        frames(&mut shelf, 41, 42);
        assert_eq!(updates(&shelf), 30, "no number holds it");
        shelf.pace(1, 1e9, TICK_MS * 42.5);
        shelf.tick(TICK_MS * 43.5);
        assert_eq!(updates(&shelf), 30 + FASTEST as u64, "at the fastest");
        shelf.tick(TICK_MS * 53.5);
        let most = CATCH_UP * FASTEST as u64;
        assert_eq!(updates(&shelf), 34 + most, "behind, it catches up as fast");
    }

    #[test]
    fn a_step_runs_updates_now_outside_the_clock() {
        let mut shelf = Shelf::default();
        let (held, buttons) = pressed(0);
        shelf.put(1, held, 0.0);
        shelf.pace(1, 0.0, 0.0);
        shelf.press(1, 0b10);
        assert_eq!(shelf.step(1, 3), 3);
        assert_eq!(given(&buttons), [0b10; 3]);
        frames(&mut shelf, 0, 9);
        assert_eq!(shelf.cart(1).unwrap().updates(), 3, "the clock holds it");
        shelf.put(2, cart(2), 0.0);
        assert_eq!(shelf.step(2, 5), 2, "it stops at the second");
        assert!(shelf.stopped(2));
        assert_eq!(shelf.step(9, 1), 0, "no cart by that name");
    }

    #[test]
    fn a_step_runs_no_more_updates_than_a_cart_keeps_the_tones_of() {
        let mut shelf = Shelf::default();
        shelf.put(1, pressed(0).0, 0.0);
        assert_eq!(shelf.step(1, u32::MAX), KEPT as u32);
        assert_eq!(shelf.step(1, 2), 2, "the next call goes on");
    }

    #[test]
    fn a_script_plays_its_buttons_then_holds_the_cart() {
        let mut shelf = Shelf::default();
        let (cart, buttons) = pressed(0);
        shelf.put(1, cart, 0.0);
        shelf.press(1, 0b1);
        shelf.script(1, [4, 5]);
        shelf.script(1, [6]);
        assert_eq!(shelf.scripted(1), Some(3));
        shelf.press(1, 0);
        assert_eq!(shelf.scripted(1), Some(3), "a button let go goes on");
        frames(&mut shelf, 0, 9);
        assert_eq!(given(&buttons), [4, 5, 6]);
        assert_eq!(shelf.scripted(1), None);
        assert!(shelf.idle(), "held where the script ends");
        shelf.script(1, Vec::new());
        assert_eq!(shelf.scripted(1), None, "a script of nothing plays nothing");
        shelf.pace(1, 1.0, TICK_MS * 9.5);
        shelf.tick(TICK_MS * 10.5);
        assert_eq!(given(&buttons), [4, 5, 6, 0], "then the keys, once let go");
    }

    #[test]
    fn a_button_down_ends_a_script_and_the_keys_play_on() {
        let mut shelf = Shelf::default();
        let (cart, buttons) = pressed(0);
        shelf.put(1, cart, 0.0);
        shelf.script(1, [1, 1, 1, 1]);
        shelf.tick(TICK_MS * 1.5);
        shelf.press(1, 0b100);
        assert_eq!(shelf.scripted(1), None);
        shelf.tick(TICK_MS * 2.5);
        assert_eq!(given(&buttons), [1, 0b100]);
        shelf.script(1, [5, 6]);
        assert_eq!(shelf.step(1, 3), 3);
        assert_eq!(given(&buttons), [1, 0b100, 5, 6, 0b100], "then the held");
        shelf.script(1, [7]);
        shelf.step(1, 1);
        shelf.tick(TICK_MS * 3.5);
        assert_eq!(
            given(&buttons).len(),
            7,
            "a script stepped out holds no clock"
        );
    }

    #[test]
    fn the_second_pad_is_pressed_and_scripted_and_no_other_bit_arrives() {
        use cartridge::button::{A, EVERY, LEFT, on};
        let mut shelf = Shelf::default();
        let (cart, buttons) = pressed(0);
        shelf.put(1, cart, 0.0);
        let second = on(2, A);
        shelf.script(1, vec![second, u32::MAX]);
        assert_eq!(shelf.step(1, 2), 2);
        shelf.press(1, LEFT | second);
        shelf.step(1, 1);
        assert_eq!(given(&buttons), [second, EVERY, LEFT | second]);
    }

    #[test]
    fn a_bit_no_pad_has_ends_no_script() {
        let mut shelf = Shelf::default();
        let (cart, buttons) = pressed(0);
        shelf.put(1, cart, 0.0);
        shelf.script(1, [4, 5]);
        shelf.press(1, 1 << 7 | 1 << 15 | 1 << 31);
        assert_eq!(shelf.scripted(1), Some(2), "no button went down");
        assert_eq!(shelf.step(1, 3), 3);
        assert_eq!(given(&buttons), [4, 5, 0], "and none is held after");
    }

    #[test]
    fn a_watched_cart_is_read_once_a_run_and_on_its_stop() {
        let mut shelf = Shelf::default();
        let watched = |trap_at| {
            Cart::watched(
                Counting {
                    updates: 0,
                    trap_at,
                    buttons: Rc::default(),
                },
                1,
            )
        };
        shelf.put(1, watched(0), 0.0);
        let watch = |shelf: &Shelf<Counting>| -> (Vec<u8>, bool, Vec<Ran>) {
            let watch = shelf.cart(1).unwrap().watch().unwrap();
            (
                watch.memory().to_vec(),
                watch.changed(1024),
                watch.ran().to_vec(),
            )
        };
        let (memory, changed, ran) = watch(&shelf);
        assert_eq!((memory.len(), memory[1024], changed), (PAGE, 0, false));
        assert!(ran.is_empty());
        shelf.press(1, 0b11);
        shelf.tick(TICK_MS * 2.5);
        let (memory, changed, ran) = watch(&shelf);
        assert_eq!((memory[1024], changed), (2, true));
        assert_eq!(
            ran,
            [Ran {
                buttons: 0b11,
                spent: 0
            }; 2],
            "the meter untouched"
        );
        let blocks = shelf.cart(1).unwrap().watch().unwrap().blocks();
        assert_eq!(blocks.len(), PAGE / BLOCK);
        assert_eq!(blocks.iter().map(|&b| usize::from(b)).sum::<usize>(), 1);
        assert_eq!(blocks[1024 / BLOCK], 1);
        shelf.put(2, watched(3), 0.0);
        shelf.step(2, 5);
        let stopped = shelf.cart(2).unwrap();
        let watch = stopped.watch().unwrap();
        assert_eq!(stopped.stop(), Some(&Stop::Trap));
        assert_eq!(
            watch.memory()[1024],
            3,
            "what the update that stopped it wrote"
        );
        assert_eq!(watch.ran().len(), 3);
        let mut frames = Frames::default();
        let mut alone = Shelf::default();
        alone.put(3, cart(3), 0.0);
        for _ in 0..3 {
            alone.step(3, 1);
            frames.add(alone.cart(3).unwrap());
        }
        assert_eq!(watch.frames(), frames, "the stop screen among them");
        assert_ne!(frames, Frames::default());
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

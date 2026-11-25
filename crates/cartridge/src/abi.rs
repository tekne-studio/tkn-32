//! The names a runner knows a cartridge by, and the one place in this crate
//! where `unsafe` is written: to import `tone`; in the exports
//! [`cart!`](crate::cart) writes into a game's crate, to give each function
//! its name; and in [`cover!`](crate::cover), to place the cover in its
//! custom section. No pointer is read here: `frame` and `palette`
//! hand out the address of a static, which the runner reads within the
//! cartridge's memory, or refuses.

use spin::Mutex;

use crate::{Buttons, Game, LOUDEST, Palette, Screen};

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
#[allow(unsafe_code)]
unsafe extern "C" {
    /// The runner's `tone`, as the contract names it.
    #[link_name = "tone"]
    safe fn env_tone(
        channel: u32,
        hz: u32,
        ms: u32,
        wave: u32,
        to_hz: u32,
        loud: u32,
        to_loud: u32,
    );
}

/// Plays `hz` for `ms` milliseconds on `channel` in `wave` at the loudest,
/// replacing what the channel plays; `hz` 0 silences it. Natively, as in a
/// test, nothing is heard.
pub fn tone(channel: u32, hz: u32, ms: u32, wave: u32) {
    tone_to(channel, hz, ms, wave, hz, LOUDEST, LOUDEST);
}

/// A tone that moves: as [`tone`], its pitch sliding by equal ratios from
/// `hz` to `to_hz` and its loudness in a straight line from `loud` to
/// `to_loud` over its `ms`, each out of [`LOUDEST`], a [`tone`]'s. On
/// noise the pitch is how high it reaches. Both loudnesses 0 silence the
/// channel, as `hz` 0 does. Natively, as in a test, nothing is heard.
pub fn tone_to(channel: u32, hz: u32, ms: u32, wave: u32, to_hz: u32, loud: u32, to_loud: u32) {
    #[cfg(target_arch = "wasm32")]
    env_tone(channel, hz, ms, wave, to_hz, loud, to_loud);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (channel, hz, ms, wave, to_hz, loud, to_loud);
}

/// Where a game lives between two calls.
pub struct Slot<G> {
    game: Mutex<Option<G>>,
}

impl<G: Game> Slot<G> {
    pub const fn new() -> Self {
        Slot {
            game: Mutex::new(None),
        }
    }

    pub fn init(&self, shown: &Shown, seed: u32) {
        let game = G::start(seed);
        *shown.palette.lock() = game.palette();
        *self.game.lock() = Some(game);
    }

    pub fn update(&self, shown: &Shown, buttons: u32) {
        if let Some(game) = self.game.lock().as_mut() {
            game.update(Buttons(buttons), &mut shown.screen.lock());
            *shown.palette.lock() = game.palette();
        }
    }
}

impl<G: Game> Default for Slot<G> {
    fn default() -> Self {
        Slot::new()
    }
}

/// The screen and the palette a runner reads. Apart from the game, so they
/// start as zeros and cost the file nothing.
pub struct Shown {
    screen: Mutex<Screen>,
    palette: Mutex<Palette>,
}

impl Shown {
    pub const fn new() -> Self {
        Shown {
            screen: Mutex::new(Screen::new()),
            palette: Mutex::new([[0; 4]; crate::COLOURS]),
        }
    }

    pub fn frame(&self) -> *const u8 {
        self.screen.lock().pixels().as_ptr()
    }

    pub fn palette(&self) -> *const u8 {
        self.palette.lock().as_flattened().as_ptr()
    }
}

impl Default for Shown {
    fn default() -> Self {
        Shown::new()
    }
}

/// Exports a [`Game`] by the contract's names, and gives a `no_std`
/// cartridge its panic handler: a panic traps, and the runner stops the
/// cartridge. Natively it writes nothing, so a game's crate also builds and
/// tests as plain Rust.
///
/// ```ignore
/// #![cfg_attr(target_arch = "wasm32", no_std)]
/// cartridge::cart!(MyGame);
/// ```
#[macro_export]
macro_rules! cart {
    ($game:ty) => {
        #[cfg(target_arch = "wasm32")]
        static CART: $crate::abi::Slot<$game> = $crate::abi::Slot::new();
        #[cfg(target_arch = "wasm32")]
        static SHOWN: $crate::abi::Shown = $crate::abi::Shown::new();

        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn cart_abi() -> u32 {
            $crate::ABI
        }

        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn init(seed: u32) {
            CART.init(&SHOWN, seed)
        }

        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn update(buttons: u32) {
            CART.update(&SHOWN, buttons)
        }

        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn frame() -> *const u8 {
            SHOWN.frame()
        }

        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn palette() -> *const u8 {
            SHOWN.palette()
        }

        #[cfg(target_arch = "wasm32")]
        #[panic_handler]
        fn panic(_: &core::panic::PanicInfo) -> ! {
            core::arch::wasm32::unreachable()
        }
    };
}

/// Says in the file how many pads the game plays, 1 to
/// [`button::PADS`](crate::button::PADS), in its custom section
/// [`button::PADS_SECTION`](crate::button::PADS_SECTION): a host reads it
/// without running the game, to seat as many people. A game without it
/// plays one; another count stops the build. Its static takes a name of
/// its own, so a game imports the contract's names beside it.
///
/// ```
/// use cartridge::button::{PADS, PADS_SECTION};
///
/// cartridge::pads!(PADS);
/// # fn main() { assert_eq!(PADS_SECTION, "cartridge.pads") }
/// ```
#[macro_export]
macro_rules! pads {
    ($pads:expr) => {
        const _: () = assert!(
            1 <= $pads && $pads <= $crate::button::PADS,
            "a game plays 1 to cartridge::button::PADS pads"
        );

        // The section's name is `cartridge::button::PADS_SECTION`: an
        // attribute takes a literal. On wasm32 rustc writes the static's
        // bytes as the custom section whether or not anything reads it.
        #[cfg_attr(
            target_arch = "wasm32",
            allow(unsafe_code),
            unsafe(link_section = "cartridge.pads")
        )]
        #[allow(dead_code)]
        static PADS_PLAYED: [u8; 1] = [$pads as u8];
    };
}

/// Places the cover `cartridge_build::cover` wrote for this crate twice: in
/// the file's `cartridge` custom section, which a host reads without
/// running the game, and in the game's data as `COVER`, a
/// [`Cover`](crate::Cover) the game draws and whose
/// [`palette`](crate::Cover::palette) it gives while it shows it. The data
/// copy is placed as the file is instantiated, off the meter; a section the
/// reader would refuse stops the build.
///
/// ```ignore
/// // build.rs: cartridge_build::cover("cover.png", "DOT", "TEKNE STUDIO", 2026);
/// cartridge::cover!();
/// // in an update: COVER.draw(screen);
/// ```
#[macro_export]
macro_rules! cover {
    () => {
        const COVER_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cover.cartridge"));

        // The section's name is `cartridge::cover::SECTION`: an attribute
        // takes a literal. On wasm32 rustc writes the static's bytes as the
        // custom section whether or not anything reads it, so no `#[used]`:
        // it would keep a third copy, the static itself, in the data.
        #[cfg_attr(
            target_arch = "wasm32",
            allow(unsafe_code),
            unsafe(link_section = "cartridge")
        )]
        #[allow(dead_code)]
        static COVER_SECTION: [u8; COVER_BYTES.len()] = *COVER_BYTES.first_chunk().unwrap();

        /// The game's cover, a copy of its section in its data.
        pub static COVER: $crate::Cover<'static> = match $crate::Cover::read(COVER_BYTES) {
            Some(cover) => cover,
            None => panic!(
                "the cover is not one cartridge::Cover reads: write it with cartridge_build::cover"
            ),
        };
    };
}

//! The names a host knows the runner by: what it asks of the host's engine,
//! the imports of module `engine`, and what the host calls, its exports.
//! Everything crosses as numbers: bytes into the runner go through a room
//! the host writes ([`room`], [`name`]), and bytes out come back at [`out`],
//! the call that gave them saying how many, -1 when no cart is there by the
//! name asked. Carts are named by the host, any `u32`; time is the host's
//! clock in milliseconds, one clock for every call.
//!
//! The engine holds the games: [`load`] compiles and instantiates a
//! prepared file, its `env.tone` import given the runner's own [`hear`]
//! (named by the last two lines of [`names`]), and the runner calls a
//! game's functions and reads its memory through the other imports. A host
//! keeps no rule of its own: it draws a screen by looking each pixel up
//! ([`shown`]), and sounds what [`sound`] gives as the chip takes it.
#![allow(unsafe_code)]

use cartridge::font::{ADVANCE, GLYPH, glyph};
use cartridge::{
    ABI, FRAME_OVERHEAD, HEIGHT, MAX_FILE, MAX_FRAME, MAX_MEMORY, MAX_STACK, MAX_TABLE, MAX_TONES,
    TICKS, UPDATE_BUDGET, WIDTH,
};
use cartridge_run::{BLOCK, Cart, KEPT, KEYS, LONGEST, SHARED, SHOWN_BYTES, Stop, WORDS, Watch};

/// The version of what the runner asks of a host's engine, the imports of
/// module `engine` ([`engine`]): a host gives the runner the version it
/// was written for, and refuses another.
const ENGINE: u32 = 1;

use crate::{Game, Runner, started};

/// What the runner asks of the host's engine.
pub(crate) mod engine {
    use cartridge_run::Trapped;

    #[link(wasm_import_module = "engine")]
    unsafe extern "C" {
        /// Compiles and instantiates the prepared file of `len` bytes at `at`:
        /// a handle, 0 or more, or -1 when the engine would not.
        #[link_name = "load"]
        fn engine_load(at: *const u8, len: usize) -> i32;
        /// Writes, up to `max` bytes at `at`, the engine's words for why the
        /// last load failed, as UTF-8: how many it wrote.
        #[link_name = "why_not"]
        fn engine_why_not(at: *mut u8, max: usize) -> usize;
        /// Calls the function of `game` named by line `function` of [`names`](super::names),
        /// with `arg` when it takes one: 0 when it returned, its result (0 for
        /// none, or for a function the game lacks) written at `out`; 1 when it
        /// trapped or ran out of stack.
        #[link_name = "call"]
        fn engine_call(game: i32, function: u32, arg: u32, out: *mut u32) -> i32;
        /// Copies `len` bytes of `game`'s memory from `from` to `at`: 1, or 0
        /// when any of them lies outside its memory, nothing copied.
        #[link_name = "read"]
        fn engine_read(game: i32, from: u32, at: *mut u8, len: usize) -> i32;
        /// The value of `game`'s meter.
        #[link_name = "meter"]
        fn engine_meter(game: i32) -> i32;
        #[link_name = "set_meter"]
        fn engine_set_meter(game: i32, units: i32);
        /// Lets `game` go: its handle names nothing after.
        #[link_name = "drop"]
        fn engine_drop(game: i32);
    }

    pub(crate) fn load(prepared: &[u8]) -> Option<i32> {
        let handle = unsafe { engine_load(prepared.as_ptr(), prepared.len()) };
        (handle >= 0).then_some(handle)
    }

    pub(crate) fn why_not() -> String {
        let mut words = vec![0; 512];
        let len = unsafe { engine_why_not(words.as_mut_ptr(), words.len()) };
        words.truncate(len.min(words.len()));
        String::from_utf8_lossy(&words).into_owned()
    }

    pub(crate) fn call(game: i32, function: u32, arg: u32) -> Result<u32, Trapped> {
        let mut out = 0;
        match unsafe { engine_call(game, function, arg, &mut out) } {
            0 => Ok(out),
            _ => Err(Trapped),
        }
    }

    pub(crate) fn read(game: i32, from: u32, into: &mut [u8]) -> bool {
        unsafe { engine_read(game, from, into.as_mut_ptr(), into.len()) == 1 }
    }

    pub(crate) fn meter(game: i32) -> i32 {
        unsafe { engine_meter(game) }
    }

    pub(crate) fn set_meter(game: i32, units: i32) {
        unsafe { engine_set_meter(game, units) }
    }

    pub(crate) fn let_go(game: i32) {
        unsafe { engine_drop(game) }
    }
}

/// The names a host finds a prepared game's parts by, a line each, at
/// [`out`] ([`crate::names`]).
#[unsafe(no_mangle)]
pub extern "C" fn names() -> i32 {
    Runner::with(|runner| runner.give(crate::names().as_bytes()))
}

/// The version of the contract the runner runs games of.
#[unsafe(no_mangle)]
pub extern "C" fn contract() -> u32 {
    ABI
}

/// The updates a second a cart runs at its clock's pace of 1.
#[unsafe(no_mangle)]
pub extern "C" fn ticks() -> u32 {
    TICKS
}

/// The most updates whose tones a cart keeps until [`sound`] takes them.
#[unsafe(no_mangle)]
pub extern "C" fn kept() -> u32 {
    KEPT as u32
}

/// The most updates a script holds ([`script_room`]), the longest session
/// the console keeps.
#[unsafe(no_mangle)]
pub extern "C" fn longest() -> u32 {
    LONGEST as u32
}

/// The words a tone is, in [`sound`]'s batches and [`hear`]'s calls.
#[unsafe(no_mangle)]
pub extern "C" fn words() -> u32 {
    WORDS as u32
}

/// The keys one person plays the buttons by (`cartridge_run::KEYS`), at
/// [`out`]: a line each, the key's name, a space, its button's bit.
#[unsafe(no_mangle)]
pub extern "C" fn keys() -> i32 {
    give_keys(&KEYS)
}

/// The keys two people sharing a keyboard play the buttons by, a half each
/// (`cartridge_run::SHARED`), at [`out`], as [`keys`] gives them.
#[unsafe(no_mangle)]
pub extern "C" fn shared() -> i32 {
    give_keys(&SHARED)
}

fn give_keys(table: &[(&str, u32)]) -> i32 {
    let lines: Vec<String> = table
        .iter()
        .map(|(key, button)| format!("{key} {button}"))
        .collect();
    Runner::with(|runner| runner.give(lines.join("\n").as_bytes()))
}

/// The pads `update`'s number carries, a byte each from its lowest.
#[unsafe(no_mangle)]
pub extern "C" fn pads() -> u32 {
    cartridge::button::PADS
}

/// How far each pad's byte lies above the one before it, in bits.
#[unsafe(no_mangle)]
pub extern "C" fn pad_bits() -> u32 {
    cartridge::button::PAD_BITS
}

/// The contract's buttons, at [`out`]: a line each, the button's name, a
/// space, its bit in the first pad's byte.
#[unsafe(no_mangle)]
pub extern "C" fn buttons() -> i32 {
    let lines: Vec<String> = cartridge::button::NAMED
        .iter()
        .map(|(name, bit)| format!("{name} {bit}"))
        .collect();
    Runner::with(|runner| runner.give(lines.join("\n").as_bytes()))
}

/// A screen's pixels across.
#[unsafe(no_mangle)]
pub extern "C" fn width() -> u32 {
    WIDTH as u32
}

/// A screen's pixels down.
#[unsafe(no_mangle)]
pub extern "C" fn height() -> u32 {
    HEIGHT as u32
}

/// The meter's units one update may spend.
#[unsafe(no_mangle)]
pub extern "C" fn budget() -> u32 {
    UPDATE_BUDGET
}

/// The bytes of memory one byte of [`blocks`] stands for.
#[unsafe(no_mangle)]
pub extern "C" fn block() -> u32 {
    BLOCK as u32
}

/// The version of the imports the runner asks of a host's engine.
#[unsafe(no_mangle)]
pub extern "C" fn engine() -> u32 {
    ENGINE
}

/// The most bytes a cartridge's file may have.
#[unsafe(no_mangle)]
pub extern "C" fn most() -> u32 {
    MAX_FILE as u32
}

/// The calls to `env.tone` an update may make that count: a host that hears
/// a game apart from the runner hands [`hear`] at most that many a call.
#[unsafe(no_mangle)]
pub extern "C" fn tones() -> u32 {
    MAX_TONES as u32
}

/// The most pages of 64 KiB a game's memory has: a host's engine lets a
/// game hold no more, whatever its file declares.
#[unsafe(no_mangle)]
pub extern "C" fn pages() -> u32 {
    (MAX_MEMORY / 65_536) as u32
}

/// The most elements of a game's one table.
#[unsafe(no_mangle)]
pub extern "C" fn table() -> u32 {
    MAX_TABLE
}

/// The deepest chain of calls a game reaches before its own count stops
/// it: a host's engine holds at least as many, or it stops a game the
/// contract lets run.
#[unsafe(no_mangle)]
pub extern "C" fn calls() -> u32 {
    MAX_STACK / FRAME_OVERHEAD
}

/// The most values that chain holds, the frame of the call that starts it
/// beside: a host's engine holds at least as many.
#[unsafe(no_mangle)]
pub extern "C" fn values() -> u32 {
    MAX_STACK + MAX_FRAME
}

/// The line of text in the room in the console's font, at [`out`]: its
/// width and its height, two bytes each, little end first, then a byte a
/// pixel, a row after another, 1 where the font inks. A letter the font
/// lacks keeps its place, blank.
#[unsafe(no_mangle)]
pub extern "C" fn text() -> i32 {
    Runner::with(|runner| {
        let line = String::from_utf8_lossy(&runner.room).into_owned();
        let count = line.chars().count() as i32;
        let width = (count * ADVANCE[0] - 1).max(0);
        let height = GLYPH[1];
        let mut bytes = Vec::with_capacity(4 + (width * height) as usize);
        bytes.extend((width as u16).to_le_bytes());
        bytes.extend((height as u16).to_le_bytes());
        let at = bytes.len();
        bytes.resize(at + (width * height) as usize, 0);
        for (i, c) in (0..).zip(line.chars()) {
            let Some(rows) = glyph(c) else { continue };
            for (row, bits) in (0..).zip(rows) {
                for col in (0..GLYPH[0]).filter(|col| bits & (0b100 >> col) != 0) {
                    bytes[at + (row * width + i * ADVANCE[0] + col) as usize] = 1;
                }
            }
        }
        runner.give(&bytes)
    })
}

/// Room for `len` bytes, for the host to write what the next call reads:
/// prepared bytes. A cartridge's file goes through [`file`], a script's
/// buttons through [`script_room`].
#[unsafe(no_mangle)]
pub extern "C" fn room(len: usize) -> *mut u8 {
    Runner::with(|runner| {
        runner.over = None;
        runner.room.clear();
        runner.room.resize(len, 0);
        runner.room.as_mut_ptr()
    })
}

/// Room for a cartridge's file of `len` bytes, for the next call that
/// reads one: [`check`], [`prepare`], [`open`], [`still`] or [`cover`]. A
/// file past [`most`] gets none, a null pointer, and is refused by its
/// size: its bytes never enter the runner, whose memory never shrinks.
#[unsafe(no_mangle)]
pub extern "C" fn file(len: usize) -> *mut u8 {
    Runner::with(|runner| {
        runner.room.clear();
        if len > MAX_FILE {
            runner.over = Some(len);
            return std::ptr::null_mut();
        }
        runner.over = None;
        runner.room.resize(len, 0);
        runner.room.as_mut_ptr()
    })
}

/// Room for the `len` bytes of the name of the file in the room, the hash
/// of its bytes, which a file with no cover shows the start of.
#[unsafe(no_mangle)]
pub extern "C" fn name(len: usize) -> *mut u8 {
    Runner::with(|runner| {
        runner.name.clear();
        runner.name.resize(len, 0);
        runner.name.as_mut_ptr()
    })
}

/// Where the bytes the last call gave lie, until the next call that gives
/// any: the host copies them out first.
#[unsafe(no_mangle)]
pub extern "C" fn out() -> *const u8 {
    Runner::with(|runner| runner.out.as_ptr())
}

/// Whether the file in the room keeps the contract: the code of why not
/// (`cartridge_run::Refusal::code`), or none when it does.
#[unsafe(no_mangle)]
pub extern "C" fn check() -> i32 {
    Runner::with(|runner| {
        let refused = runner.file().and_then(cartridge_run::prepare).err();
        let code = refused.as_ref().map(cartridge_run::Refusal::code);
        runner.give(code.unwrap_or_default().as_bytes())
    })
}

/// The file in the room prepared, for a host that keeps prepared bytes by
/// the file's hash; -1 when it breaks the contract, which [`open`] shows.
#[unsafe(no_mangle)]
pub extern "C" fn prepare() -> i32 {
    Runner::with(
        |runner| match runner.file().and_then(cartridge_run::prepare) {
            Ok(prepared) => runner.give(prepared.bytes()),
            Err(_) => -1,
        },
    )
}

/// Opens the file in the room as `id`, started with `seed`, its clock
/// running from `now`, in the place of any cart of that name: prepared,
/// loaded and started, or stopped with its screen saying why. A cart
/// `watched` is seen from inside ([`window`], [`ran`], [`frames`]).
#[unsafe(no_mangle)]
pub extern "C" fn open(id: u32, seed: u32, watched: u32, now: f64) {
    Runner::with(|runner| {
        let cart = match runner.file().and_then(cartridge_run::prepare) {
            Ok(prepared) => started(Game::load(prepared.bytes()), seed, watched != 0),
            Err(refusal) => Cart::stopped(Stop::Refused(refusal)),
        };
        runner.shelf.put(id, cart, now);
    });
}

/// [`open`] for bytes [`prepare`] gave, kept by the host. Bytes of
/// another shape stop as a load that failed, before the engine sees them
/// (`cartridge_run::shaped`); bytes of its shape that preparing never gave
/// may run unmetered, so a host keeps them only where no one else writes.
#[unsafe(no_mangle)]
pub extern "C" fn open_prepared(id: u32, seed: u32, watched: u32, now: f64) {
    Runner::with(|runner| {
        let loaded = cartridge_run::shaped(&runner.room)
            .map_err(|refusal| Stop::Load(format!("not prepared bytes: {refusal}")))
            .and_then(|()| Game::load(&runner.room));
        let cart = started(loaded, seed, watched != 0);
        runner.shelf.put(id, cart, now);
    });
}

/// Shows the cover of the file in the room as `id`, still: it never runs.
#[unsafe(no_mangle)]
pub extern "C" fn still(id: u32, now: f64) {
    Runner::with(|runner| {
        let cart = cartridge_run::still(&runner.room, runner.name());
        runner.shelf.put(id, cart, now);
    });
}

/// Shows `id` as a cart whose file never came: its stop screen.
#[unsafe(no_mangle)]
pub extern "C" fn fail(id: u32, now: f64) {
    Runner::with(|runner| {
        runner.shelf.put(id, Cart::stopped(Stop::Missing), now);
    });
}

/// Takes `id` off the shelf, its game let go.
#[unsafe(no_mangle)]
pub extern "C" fn close(id: u32) {
    Runner::with(|runner| runner.shelf.remove(id));
}

/// Holds the contract's `buttons`, every pad's, down on `id` from its next
/// update on.
#[unsafe(no_mangle)]
pub extern "C" fn press(id: u32, buttons: u32) {
    Runner::with(|runner| runner.shelf.press(id, buttons));
}

/// Runs `id`'s clock at `pace` times the contract's from `now`; 0 holds it.
#[unsafe(no_mangle)]
pub extern "C" fn pace(id: u32, pace: f64, now: f64) {
    Runner::with(|runner| runner.shelf.pace(id, pace, now));
}

/// Room for `count` updates of buttons, a word each, the number `update`
/// takes, for the host to write what the next [`script`] plays. Counted
/// in words, a room holds whole numbers only, and the shelf takes them
/// where the host wrote them: a script lies in the runner's memory once.
/// A script past [`longest`] gets none, a null pointer, and the next
/// [`script`] plays nothing.
#[unsafe(no_mangle)]
pub extern "C" fn script_room(count: usize) -> *mut u32 {
    Runner::with(|runner| {
        runner.script.clear();
        if count > LONGEST {
            return std::ptr::null_mut();
        }
        runner.script.resize(count, 0);
        runner.script.as_mut_ptr()
    })
}

/// Plays the buttons in [`script_room`] on `id`, one an update, after any
/// script it plays.
#[unsafe(no_mangle)]
pub extern "C" fn script(id: u32) {
    Runner::with(|runner| {
        let buttons = std::mem::take(&mut runner.script);
        runner.shelf.script(id, buttons);
    });
}

/// How many updates of `id`'s script are left; -1 when it plays none.
#[unsafe(no_mangle)]
pub extern "C" fn scripted(id: u32) -> i32 {
    Runner::with(|runner| runner.shelf.scripted(id).map_or(-1, |left| left as i32))
}

/// Runs `id`'s next `count` updates now, outside its clock, at most
/// [`kept`] a call: how many ran.
#[unsafe(no_mangle)]
pub extern "C" fn step(id: u32, count: u32) -> u32 {
    Runner::with(|runner| runner.shelf.step(id, count))
}

/// Runs every cart's updates due by `now`: the names of the carts whose
/// screens changed, four bytes each.
#[unsafe(no_mangle)]
pub extern "C" fn tick(now: f64) -> i32 {
    Runner::with(|runner| {
        let changed = runner.shelf.tick(now);
        let bytes: Vec<u8> = changed.iter().flat_map(|id| id.to_le_bytes()).collect();
        runner.give(&bytes)
    })
}

/// 1 when a tick would do nothing: no cart runs by its clock and every
/// screen was said.
#[unsafe(no_mangle)]
pub extern "C" fn idle() -> u32 {
    Runner::with(|runner| u32::from(runner.shelf.idle()))
}

/// 1 when `id` stopped for good.
#[unsafe(no_mangle)]
pub extern "C" fn stopped(id: u32) -> u32 {
    Runner::with(|runner| u32::from(runner.shelf.stopped(id)))
}

/// How many updates `id` ran; -1 when no cart is there.
#[unsafe(no_mangle)]
pub extern "C" fn updates(id: u32) -> f64 {
    Runner::with(|runner| {
        runner
            .shelf
            .cart(id)
            .map_or(-1.0, |cart| cart.updates() as f64)
    })
}

/// What a host draws `id`'s screen from (`cartridge_run::shown`): 256
/// colours of four bytes, resolved and opaque, a pixel's byte naming one,
/// then the pixels.
#[unsafe(no_mangle)]
pub extern "C" fn shown(id: u32) -> i32 {
    Runner::with(|runner| runner.give_from(SHOWN_BYTES, |shelf, into| shelf.shown(id, into)))
}

/// `id`'s screen as the game wrote it, what an agent reads
/// (`cartridge_run::read_bytes`): its palette, then its pixels.
#[unsafe(no_mangle)]
pub extern "C" fn read(id: u32) -> i32 {
    Runner::with(|runner| match runner.shelf.read(id) {
        Some(bytes) => runner.give(&bytes),
        None => -1,
    })
}

/// What `id` played since last asked, as the chip takes it
/// (`cartridge_run::batch`), four bytes a word; none when it ran no update.
#[unsafe(no_mangle)]
pub extern "C" fn sound(id: u32) -> i32 {
    Runner::with(|runner| {
        let words = runner.shelf.sound(id);
        let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        runner.give(&bytes)
    })
}

/// Why `id` stopped, as its screen says it; none while it runs.
#[unsafe(no_mangle)]
pub extern "C" fn stop(id: u32) -> i32 {
    Runner::with(|runner| {
        let Some(cart) = runner.shelf.cart(id) else {
            return -1;
        };
        let said = cart.stop().map(ToString::to_string).unwrap_or_default();
        runner.give(said.as_bytes())
    })
}

/// The frames hash of a watched `id`, as text, the one the headless runner
/// prints for the same seed and buttons.
#[unsafe(no_mangle)]
pub extern "C" fn frames(id: u32) -> i32 {
    watched(id, |watch| watch.frames().to_string().into_bytes())
}

/// Every update a watched `id` ran from update `from`: its buttons and the
/// units it spent, four bytes each.
#[unsafe(no_mangle)]
pub extern "C" fn ran(id: u32, from: u32) -> i32 {
    watched(id, |watch| {
        let ran = watch.ran().get(from as usize..).unwrap_or_default();
        ran.iter()
            .flat_map(|ran| [ran.buttons, ran.spent])
            .flat_map(u32::to_le_bytes)
            .collect()
    })
}

/// The bytes of memory a watched `id` holds, as last read; -1 when it is
/// not watched.
#[unsafe(no_mangle)]
pub extern "C" fn size(id: u32) -> i32 {
    Runner::with(|runner| {
        let watch = runner.shelf.cart(id).and_then(Cart::watch);
        watch.map_or(-1, |watch| watch.memory().len() as i32)
    })
}

/// `len` bytes of a watched `id`'s memory from `at`, cut where it ends,
/// then a byte each, 1 where the last updates changed it.
#[unsafe(no_mangle)]
pub extern "C" fn window(id: u32, at: u32, len: u32) -> i32 {
    watched(id, |watch| {
        let memory = watch.memory();
        let from = (at as usize).min(memory.len());
        let to = from.saturating_add(len as usize).min(memory.len());
        let mut bytes = memory[from..to].to_vec();
        bytes.extend((from..to).map(|at| u8::from(watch.changed(at))));
        bytes
    })
}

/// A watched `id`'s memory's map: a byte a [`block`], 1 where the last
/// updates changed any byte of it.
#[unsafe(no_mangle)]
pub extern "C" fn blocks(id: u32) -> i32 {
    watched(id, |watch| watch.blocks())
}

/// Gives what `give` reads of a watched `id`; -1 when it is not watched.
fn watched(id: u32, give: impl FnOnce(&Watch) -> Vec<u8>) -> i32 {
    Runner::with(|runner| {
        let Some(watch) = runner.shelf.cart(id).and_then(|cart| cart.watch()) else {
            return -1;
        };
        let bytes = give(watch);
        runner.give(&bytes)
    })
}

/// What a page shows of the file in the room before it runs, read from it
/// and never by running it: its picture as [`shown`] gives a screen, then,
/// when the file has a cover, its year, two bytes, and its title and maker,
/// each a byte of length and the text.
#[unsafe(no_mangle)]
pub extern "C" fn cover() -> i32 {
    Runner::with(|runner| {
        let read = cartridge_run::cover(&runner.room);
        let mut bytes = vec![0; SHOWN_BYTES];
        let (palette, screen) = cartridge_run::cover_screen(read.as_ref(), runner.name());
        cartridge_run::shown(screen.pixels(), &palette, &mut bytes);
        if let Some(read) = read {
            bytes.extend(read.year().to_le_bytes());
            for text in [read.title(), read.maker()] {
                bytes.push(text.len() as u8);
                bytes.extend(text.as_bytes());
            }
        }
        runner.give(&bytes)
    })
}

/// A game's `env.tone`: what the game being called plays.
#[unsafe(no_mangle)]
pub extern "C" fn hear(
    channel: u32,
    hz: u32,
    ms: u32,
    wave: u32,
    to_hz: u32,
    loud: u32,
    to_loud: u32,
) {
    crate::HEARD.with_borrow_mut(|heard| heard.hear(channel, hz, ms, wave, to_hz, loud, to_loud));
}

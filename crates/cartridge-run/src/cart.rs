//! A running cartridge held to the contract, on any engine. The engine is
//! behind [`Instance`]: a handful of calls any WebAssembly engine gives, the
//! native one over wasmi, the browser's over its own `WebAssembly`. Every
//! rule is here, so a cartridge behaves the same on each.

use std::fmt;

use cartridge::{
    ABI, COLOURS, Export, MAX_MEMORY, PIXELS, Palette, SCREENS, Screen, UPDATE_BUDGET, WIDTH,
    button,
};

use crate::prepare::{OUT_OF_MEMORY, Refusal, TOO_DEEP};
use crate::tones::Tone;
use crate::watch::{Ran, Watch};

/// The call trapped. Why is read from the meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trapped;

/// A prepared cartridge instantiated on one engine. An engine instantiates
/// it its own way, at once or later, and hands it to [`Cart::start`].
pub trait Instance {
    /// Calls the start function the prepared file exports as
    /// [`START`](crate::START), if it has one.
    fn start(&mut self) -> Result<(), Trapped>;
    /// Calls `export`, with `arg` as `init`'s seed or `update`'s buttons
    /// (the others take none), and gives its result, or 0 for one that
    /// gives none or an optional export the file does not have: its
    /// default.
    fn call(&mut self, export: Export, arg: u32) -> Result<u32, Trapped>;
    /// Copies memory from `at` into `into`; `false` when any of it lies
    /// outside the memory.
    fn read(&self, at: u32, into: &mut [u8]) -> bool;
    /// The meter's units left; [`OUT_OF_MEMORY`] after memory was asked for
    /// past the ceiling.
    fn meter(&self) -> i32;
    fn set_meter(&mut self, units: i32);
    /// Hands over what the engine's `env.tone` heard, into [`Tones`](crate::Tones),
    /// since last asked.
    fn take_tones(&mut self) -> Vec<Tone>;
}

/// Why a cartridge stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Its file never came: the host could not fetch or read it, time
    /// after time, and gave it up. Why each try failed is the host's log.
    Missing,
    /// Its file was refused before any engine saw it.
    Refused(Refusal),
    /// The engine would not compile or instantiate the prepared file: the
    /// engine's words. Preparation leaves an engine no reason to.
    Load(String),
    /// It speaks a version of the contract this runner does not.
    Abi(u32),
    /// It asks for a step of screen past those listed.
    Size(u32),
    /// A call worked past its budget.
    Budget,
    /// It asked for memory past [`MAX_MEMORY`].
    Memory,
    /// Its calls held more than [`MAX_STACK`](cartridge::MAX_STACK) at once.
    Stack,
    /// It trapped on its own: a panic, an access outside its memory, a
    /// stack too deep.
    Trap,
    /// Its screen or its palette lies outside its memory.
    Screen,
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stop::Missing => write!(f, "its file did not come"),
            Stop::Refused(refusal) => write!(f, "refused: {refusal}"),
            Stop::Load(why) => write!(f, "it would not start: {why}"),
            Stop::Abi(abi) => write!(f, "it speaks contract {abi}, this runner {ABI}"),
            Stop::Size(step) => write!(
                f,
                "it asks for screen {step}, this runner knows 0 to {}",
                SCREENS - 1
            ),
            Stop::Budget => write!(f, "it worked past its budget"),
            Stop::Memory => write!(f, "it asked for memory past {} KB", MAX_MEMORY / 1024),
            Stop::Stack => write!(f, "it called deeper than its stack"),
            Stop::Trap => write!(f, "it trapped"),
            Stop::Screen => write!(f, "its screen lies outside its memory"),
        }
    }
}

/// A cartridge running, or stopped with the screen saying why.
pub struct Cart<I> {
    instance: Option<I>,
    screen: Box<Screen>,
    palette: Palette,
    stop: Option<Stop>,
    updates: u64,
    spent: u32,
    tones: Vec<Tone>,
    watch: Option<Box<Watch>>,
}

impl<I: Instance> Cart<I> {
    /// Runs the start function, checks the contract's version, asks for
    /// the step of its screen and calls `init` with `seed`, all under the
    /// start budget.
    pub fn start(instance: I, seed: u32) -> Cart<I> {
        let mut cart = Cart::blank();
        cart.begin_on(instance, seed);
        cart
    }

    /// [`Cart::start`], the cart seen from inside ([`Watch`]) from before
    /// its first update.
    pub fn watched(instance: I, seed: u32) -> Cart<I> {
        let mut cart = Cart::blank();
        cart.watch = Some(Box::default());
        cart.begin_on(instance, seed);
        cart
    }

    fn begin_on(&mut self, mut instance: I, seed: u32) {
        let begun = Cart::begin(&mut instance, seed)
            .map_err(|Trapped| why(&instance))
            .and_then(|ready| ready)
            .and_then(|()| {
                instance.take_tones();
                self.show(&mut instance)
            });
        if let Some(watch) = &mut self.watch {
            watch.first(&instance);
        }
        match begun {
            Ok(()) => self.instance = Some(instance),
            Err(stop) => self.halt(stop),
        }
    }

    /// The calls before the first update: a trap, a stop the contract
    /// gives, or the cartridge ready.
    fn begin(instance: &mut I, seed: u32) -> Result<Result<(), Stop>, Trapped> {
        instance.start()?;
        let abi = instance.call(Export::CartAbi, 0)?;
        if abi != ABI {
            return Ok(Err(Stop::Abi(abi)));
        }
        let step = instance.call(Export::Screen, 0)?;
        if step >= SCREENS {
            return Ok(Err(Stop::Size(step)));
        }
        instance.call(Export::Init, seed)?;
        Ok(Ok(()))
    }

    /// A cartridge that never ran, the screen saying why.
    pub fn stopped(stop: Stop) -> Cart<I> {
        let mut cart = Cart::blank();
        cart.halt(stop);
        cart
    }

    /// A cartridge that never runs, showing `screen` in `palette`: its
    /// cover on an idle cabinet ([`crate::still`]). Not stopped, it plays
    /// nothing and its screen never changes.
    pub(crate) fn showing(palette: Palette, screen: Box<Screen>) -> Cart<I> {
        Cart {
            palette,
            screen,
            ..Cart::blank()
        }
    }

    fn blank() -> Cart<I> {
        Cart {
            instance: None,
            screen: Box::new(Screen::new()),
            palette: [[0; 4]; COLOURS],
            stop: None,
            updates: 0,
            spent: 0,
            tones: Vec::new(),
            watch: None,
        }
    }

    /// One tick: `update` with the buttons held, then the screen and the
    /// palette read, all under one budget. Gives the tones it played.
    pub fn update(&mut self, buttons: u32) -> &[Tone] {
        self.tones.clear();
        let Some(mut instance) = self.instance.take() else {
            return &self.tones;
        };
        let buttons = buttons & button::ALL;
        instance.set_meter(UPDATE_BUDGET as i32);
        let ran = match instance.call(Export::Update, buttons) {
            Err(Trapped) => Err(why(&instance)),
            Ok(_) => {
                self.tones = instance.take_tones();
                self.updates += 1;
                self.show(&mut instance)
            }
        };
        let spent = UPDATE_BUDGET.saturating_sub(instance.meter().max(0) as u32);
        if let Some(watch) = &mut self.watch {
            watch.add(Ran {
                buttons: buttons as u8,
                spent,
            });
            // The engine is let go as the cart stops: what the update wrote
            // before it is read first.
            if ran.is_err() {
                watch.look(&instance);
            }
        }
        match ran {
            Ok(()) => {
                self.spent = spent;
                self.instance = Some(instance);
            }
            Err(stop) => self.halt(stop),
        }
        if let Some(watch) = &mut self.watch {
            watch
                .frames_mut()
                .add_shown(self.screen.pixels(), &self.palette);
        }
        &self.tones
    }

    /// Reads the memory again into its [`Watch`], for the updates run since
    /// the last look to show what they changed; a cart not watched, or one
    /// stopped, keeps what it had.
    pub fn look(&mut self) {
        if let (Some(watch), Some(instance)) = (&mut self.watch, &self.instance) {
            watch.look(instance);
        }
    }

    /// What it shows of itself, when it was started [`Cart::watched`].
    pub fn watch(&self) -> Option<&Watch> {
        self.watch.as_deref()
    }

    /// Reads the screen and the palette the cartridge points at.
    fn show(&mut self, instance: &mut I) -> Result<(), Stop> {
        let frame = instance.call(Export::Frame, 0);
        let colours = frame.and_then(|frame| Ok((frame, instance.call(Export::Palette, 0)?)));
        let (frame, colours) = colours.map_err(|Trapped| why(instance))?;
        let mut palette = [0; COLOURS * 4];
        if !(instance.read(frame, self.screen.pixels_mut()) && instance.read(colours, &mut palette))
        {
            return Err(Stop::Screen);
        }
        for (colour, rgba) in self.palette.iter_mut().zip(palette.chunks_exact(4)) {
            colour.copy_from_slice(rgba);
        }
        Ok(())
    }

    /// Stops for good: the engine is let go and the screen says why.
    fn halt(&mut self, stop: Stop) {
        self.instance = None;
        (self.palette, self.screen) = stopped_screen(&stop, self.updates);
        self.stop = Some(stop);
    }

    /// The last screen, an index into the palette a pixel.
    pub fn screen(&self) -> &[u8; PIXELS] {
        self.screen.pixels()
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Whether an update runs it: it started and has not stopped. A cart
    /// stopped, or one that only shows a screen ([`crate::still`]), never
    /// changes again.
    pub fn runs(&self) -> bool {
        self.instance.is_some()
    }

    /// Why it stopped, if it did.
    pub fn stop(&self) -> Option<&Stop> {
        self.stop.as_ref()
    }

    /// The updates it has run.
    pub fn updates(&self) -> u64 {
        self.updates
    }

    /// The meter's units the last update took, the screen and the palette
    /// read with it, out of [`UPDATE_BUDGET`].
    pub fn spent(&self) -> u32 {
        self.spent
    }

    /// The screen in colour, four bytes a pixel, row by row from the top
    /// left, as [`crate::resolve`] draws it.
    pub fn rgba(&self, into: &mut [u8]) {
        crate::colour::rgba(self.screen.pixels(), &self.palette, into);
    }

    /// What a host draws the screen from, [`crate::SHOWN_BYTES`] into
    /// `into`: the palette resolved, then the pixels.
    pub fn shown(&self, into: &mut [u8]) {
        crate::colour::shown(self.screen.pixels(), &self.palette, into);
    }
}

/// The host's own screen, which a stopped cartridge and a cover's stand-in
/// are drawn on: colour 0 the plain ground, then [`INK`], [`ALERT`] and
/// [`DIM`].
pub(crate) fn host_screen() -> (Palette, Box<Screen>) {
    let mut palette = [[0, 0, 0, 255]; COLOURS];
    palette[0] = [24, 8, 14, 255];
    palette[usize::from(INK)] = [236, 236, 244, 255];
    palette[usize::from(ALERT)] = [255, 77, 106, 255];
    palette[usize::from(DIM)] = [150, 140, 160, 255];
    (palette, Box::new(Screen::new()))
}

pub(crate) const INK: u8 = 1;
pub(crate) const ALERT: u8 = 2;
pub(crate) const DIM: u8 = 3;

/// Why the last call trapped, as the meter tells it.
fn why(instance: &impl Instance) -> Stop {
    match instance.meter() {
        OUT_OF_MEMORY => Stop::Memory,
        TOO_DEEP => Stop::Stack,
        units if units < 0 => Stop::Budget,
        _ => Stop::Trap,
    }
}

/// The screen of a stopped cartridge: what stopped it, in words, after how
/// many updates.
fn stopped_screen(stop: &Stop, updates: u64) -> (Palette, Box<Screen>) {
    let (palette, mut screen) = host_screen();
    screen.text(8, 10, "STOPPED", ALERT, 2);
    let mut y = 30;
    for line in wrapped(&stop.to_string(), (WIDTH as i32 - 16) / 4) {
        screen.text(8, y, &line, INK, 1);
        y += 7;
    }
    screen.text(8, y + 6, &format!("after {updates} updates"), DIM, 1);
    (palette, screen)
}

/// `text` cut at spaces into lines of at most `width` characters; a word
/// longer than a line is cut where the line ends.
fn wrapped(text: &str, width: i32) -> Vec<String> {
    let width = width.max(1) as usize;
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        loop {
            let line = lines.last_mut().expect("never empty");
            let taken = line.chars().count();
            let room = width.saturating_sub(taken + usize::from(taken > 0));
            if word.len() <= room {
                if taken > 0 {
                    line.push(' ');
                }
                line.extend(word);
                break;
            }
            if taken == 0 {
                line.extend(word.drain(..width));
            }
            lines.push(String::new());
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::wrapped;

    #[test]
    fn words_wrap_at_spaces_and_a_long_one_is_cut() {
        assert_eq!(
            wrapped("it worked past its budget", 10),
            ["it worked", "past its", "budget"]
        );
        assert_eq!(wrapped("abcdefghijkl x", 5), ["abcde", "fghij", "kl x"]);
    }
}

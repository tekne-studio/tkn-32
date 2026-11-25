//! The emulator's runner: every cartridge a host runs, on one shelf
//! ([`Shelf`]), in a WebAssembly module of plain imports and exports
//! ([`abi`]). The host's engine compiles each game beside it and runs what
//! the runner calls; the runner prepares each file, keeps each game's clock,
//! hears what it plays, judges why it stops and resolves how its screen is
//! drawn. So every host runs the same file, and none holds a rule of its own:
//! a page's worker on the browser's WebAssembly, a native shell on its
//! interpreter, a test on either.
#![cfg(target_arch = "wasm32")]

mod abi;

use std::cell::RefCell;

use cartridge::{Export, MEMORY, TONE};
use cartridge_run::{Cart, Instance, METER, Refusal, START, Shelf, Stop, Tone, Tones, Trapped};

/// How many functions of a game a host calls, by their place in
/// [`names`]: the contract's exports in [`Export`]'s order, then the start
/// function preparing gives it.
const FUNCTIONS: usize = Export::REQUIRED.len() + Export::OPTIONAL.len() + 1;

/// The names a host finds a prepared game's parts by, a line each: the
/// functions it calls, by the index the runner calls each by, then the
/// meter's global, the memory, and the module and name of the import a
/// game plays its tones through, which the host gives the runner's `hear`.
fn names() -> String {
    let mut functions = [""; FUNCTIONS];
    for export in Export::REQUIRED.into_iter().chain(Export::OPTIONAL) {
        functions[export as usize] = export.name();
    }
    functions[FUNCTIONS - 1] = START;
    let [module, name] = TONE;
    let parts = [METER, MEMORY, module, name];
    functions
        .iter()
        .chain(&parts)
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// A game the host's engine holds for the runner, by its handle.
struct Game {
    handle: i32,
    /// What it heard in calls since its tones were last taken.
    tones: Tones,
}

thread_local! {
    static RUNNER: RefCell<Runner> = RefCell::new(Runner::default());
    /// What the game being called hears: its own tones, lent while it runs.
    static HEARD: RefCell<Tones> = RefCell::new(Tones::default());
}

impl Game {
    /// Asks the host's engine to compile and instantiate a prepared file.
    fn load(prepared: &[u8]) -> Result<Game, Stop> {
        match abi::engine::load(prepared) {
            Some(handle) => Ok(Game {
                handle,
                tones: Tones::default(),
            }),
            None => Err(Stop::Load(abi::engine::why_not())),
        }
    }

    /// Calls the function at `index` of [`names`], the game hearing into
    /// its own tones meanwhile.
    fn called(&mut self, index: usize, arg: u32) -> Result<u32, Trapped> {
        HEARD.replace(std::mem::take(&mut self.tones));
        let called = abi::engine::call(self.handle, index as u32, arg);
        self.tones = HEARD.take();
        called
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        abi::engine::let_go(self.handle);
    }
}

impl Instance for Game {
    fn start(&mut self) -> Result<(), Trapped> {
        self.called(FUNCTIONS - 1, 0).map(drop)
    }

    fn call(&mut self, export: Export, arg: u32) -> Result<u32, Trapped> {
        self.called(export as usize, arg)
    }

    fn read(&self, at: u32, into: &mut [u8]) -> bool {
        abi::engine::read(self.handle, at, into)
    }

    fn meter(&self) -> i32 {
        abi::engine::meter(self.handle)
    }

    fn set_meter(&mut self, units: i32) {
        abi::engine::set_meter(self.handle, units);
    }

    fn take_tones(&mut self) -> Vec<Tone> {
        self.tones.take()
    }
}

/// The shelf, and the bytes that cross to and from the host.
#[derive(Default)]
struct Runner {
    shelf: Shelf<Game>,
    /// What the host wrote for the next call: a file or prepared bytes.
    room: Vec<u8>,
    /// The buttons the host wrote for the next [`abi::script`], a word an
    /// update, which the shelf takes where they lie.
    script: Vec<u32>,
    /// The size of a file past the most a file may have, whose bytes the
    /// room never took ([`abi::file`]).
    over: Option<usize>,
    /// The name of the file in [`Runner::room`], for a cover.
    name: Vec<u8>,
    /// What the last call that gives bytes gave, for the host to copy out.
    out: Vec<u8>,
}

impl Runner {
    fn with<T>(work: impl FnOnce(&mut Runner) -> T) -> T {
        RUNNER.with_borrow_mut(work)
    }

    /// Gives `bytes` to the host: their length, the bytes at [`abi::out`].
    fn give(&mut self, bytes: &[u8]) -> i32 {
        self.out.clear();
        self.out.extend_from_slice(bytes);
        self.out.len() as i32
    }

    /// Gives what `fill` writes into `len` bytes, or -1 when it wrote
    /// nothing: no cart by the name asked.
    fn give_from(&mut self, len: usize, fill: impl FnOnce(&Shelf<Game>, &mut [u8]) -> bool) -> i32 {
        self.out.clear();
        self.out.resize(len, 0);
        match fill(&self.shelf, &mut self.out) {
            true => len as i32,
            false => -1,
        }
    }

    /// The file in the room, or why it is refused unread: past the most a
    /// file may have, its bytes never came in.
    fn file(&self) -> Result<&[u8], Refusal> {
        match self.over {
            Some(len) => Err(Refusal::TooLarge(len)),
            None => Ok(&self.room),
        }
    }

    fn name(&self) -> &str {
        std::str::from_utf8(&self.name).unwrap_or_default()
    }
}

/// A cart from what loading gave, started with `seed`, watched when asked.
fn started(loaded: Result<Game, Stop>, seed: u32, watched: bool) -> Cart<Game> {
    match loaded {
        Ok(game) if watched => Cart::watched(game, seed),
        Ok(game) => Cart::start(game, seed),
        Err(stop) => Cart::stopped(stop),
    }
}

//! The arcade's workbench: one cartridge held to the contract by `Cart`,
//! as a cabinet holds it, and seen from inside. The page steps it one
//! update at a time; after each it reads the screen, the palette, the
//! meter, the tones and the whole memory, and marks what the update changed.
//!
//! It sees by reading, never by asking the game: the memory is read through
//! the same [`Instance::read`] a runner reads the screen with, from an
//! instance lent to the `Cart` ([`Lent`]), so a game gets nothing a cabinet
//! does not give it. In the browser it runs on the page's own WebAssembly,
//! in a worker (`web`); natively, its tests run it on wasmi.

use std::cell::RefCell;
use std::rc::Rc;

use cartridge::{Export, MAX_MEMORY};
use cartridge_run::{Cart, Frames, Instance, Stop, Tone, Trapped};

#[cfg(target_arch = "wasm32")]
mod web;

/// A page of WebAssembly memory: how far a read reaches before the next.
const PAGE: usize = 64 * 1024;
/// The bytes one mark of the memory's map stands for.
pub const BLOCK: usize = 256;
/// An instance lent to a `Cart` while the workbench keeps a hand on it, to
/// read its memory between updates. Every call goes through as it came.
pub struct Lent<I>(Rc<RefCell<I>>);

impl<I: Instance> Instance for Lent<I> {
    fn start(&mut self) -> Result<(), Trapped> {
        self.0.borrow_mut().start()
    }

    fn call(&mut self, export: Export, arg: u32) -> Result<u32, Trapped> {
        self.0.borrow_mut().call(export, arg)
    }

    fn read(&self, at: u32, into: &mut [u8]) -> bool {
        self.0.borrow().read(at, into)
    }

    fn meter(&self) -> i32 {
        self.0.borrow().meter()
    }

    fn set_meter(&mut self, units: i32) {
        self.0.borrow_mut().set_meter(units);
    }

    fn take_tones(&mut self) -> Vec<Tone> {
        self.0.borrow_mut().take_tones()
    }
}

/// One cartridge on the bench: running or stopped, its memory as the last
/// update left it and as the one before left it.
pub struct Bench<I> {
    cart: Cart<Lent<I>>,
    /// The instance the cart runs, kept to read; let go when it stops.
    held: Option<Rc<RefCell<I>>>,
    memory: Vec<u8>,
    before: Vec<u8>,
    tones: Vec<Tone>,
    frames: Frames,
}

impl<I: Instance> Bench<I> {
    /// Starts an instance with `seed`, or shows why there is none.
    pub fn start(instance: Result<I, Stop>, seed: u32) -> Bench<I> {
        let (cart, held) = match instance {
            Ok(instance) => {
                let held = Rc::new(RefCell::new(instance));
                (Cart::start(Lent(Rc::clone(&held)), seed), Some(held))
            }
            Err(stop) => (Cart::stopped(stop), None),
        };
        let mut bench = Bench {
            cart,
            held,
            memory: Vec::new(),
            before: Vec::new(),
            tones: Vec::new(),
            frames: Frames::default(),
        };
        bench.look();
        bench.before.clone_from(&bench.memory);
        bench
    }

    /// One update with `buttons` held, then the memory read again.
    pub fn update(&mut self, buttons: u32) {
        if self.cart.stop().is_some() {
            self.tones.clear();
            self.before.clone_from(&self.memory);
            return;
        }
        self.tones = self.cart.update(buttons).to_vec();
        self.frames.add(&self.cart);
        std::mem::swap(&mut self.memory, &mut self.before);
        self.look();
    }

    /// The frames hash of the updates run, the one `bun run cart` prints
    /// for the same seed and buttons.
    pub fn frames(&self) -> Frames {
        self.frames
    }

    /// Reads the whole memory, a page at a time until a read falls outside
    /// it; a cart that stopped keeps what it held when it stopped, and its
    /// instance is let go.
    fn look(&mut self) {
        let Some(held) = &self.held else {
            return;
        };
        let instance = held.borrow();
        self.memory.clear();
        for page in 0..MAX_MEMORY / PAGE {
            let at = page * PAGE;
            self.memory.resize(at + PAGE, 0);
            if !instance.read(at as u32, &mut self.memory[at..]) {
                self.memory.truncate(at);
                break;
            }
        }
        drop(instance);
        if self.cart.stop().is_some() {
            self.held = None;
        }
    }

    pub fn cart(&self) -> &Cart<Lent<I>> {
        &self.cart
    }

    /// The tones the last update played.
    pub fn tones(&self) -> &[Tone] {
        &self.tones
    }

    /// The memory as the last update left it.
    pub fn memory(&self) -> &[u8] {
        &self.memory
    }

    /// Whether the last update changed the byte at `at`, or made the
    /// memory reach it.
    pub fn changed(&self, at: usize) -> bool {
        self.memory.get(at) != self.before.get(at)
    }

    /// The memory's map: a byte a [`BLOCK`], 1 where the last update
    /// changed any byte of it.
    pub fn blocks(&self) -> Vec<u8> {
        (0..self.memory.len().div_ceil(BLOCK))
            .map(|block| {
                let at = block * BLOCK;
                u8::from(block_of(&self.memory, at) != block_of(&self.before, at))
            })
            .collect()
    }
}

/// The block of `memory` from `at`, cut where the memory ends.
fn block_of(memory: &[u8], at: usize) -> Option<&[u8]> {
    memory.get(at..(at + BLOCK).min(memory.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cartridge_run::prepare;
    use cartridge_wasmi::Wasmi;

    /// A cart that counts its updates in the word at 1024 and draws nothing.
    const COUNTER: &str = r#"(module
        (memory (export "memory") 1 2)
        (func (export "cart_abi") (result i32) i32.const 1)
        (func (export "init") (param i32))
        (func (export "update") (param i32)
            i32.const 1024
            i32.const 1024
            i32.load
            i32.const 1
            i32.add
            i32.store)
        (func (export "frame") (result i32) i32.const 4096)
        (func (export "palette") (result i32) i32.const 0))"#;

    fn bench(wat: &str) -> Bench<Wasmi> {
        let file = wat::parse_str(wat).unwrap();
        let instance = prepare(&file)
            .map_err(Stop::Refused)
            .and_then(|prepared| Wasmi::load(&prepared));
        Bench::start(instance, 1)
    }

    #[test]
    fn an_update_marks_the_bytes_it_changed_and_no_other() {
        let mut bench = bench(COUNTER);
        assert_eq!(bench.memory().len(), PAGE);
        bench.update(0);
        bench.update(0);
        assert_eq!(bench.memory()[1024], 2);
        assert!(bench.changed(1024));
        assert!(!bench.changed(1025) && !bench.changed(0));
        let blocks = bench.blocks();
        assert_eq!(blocks.len(), PAGE / BLOCK);
        assert_eq!(blocks.iter().filter(|&&b| b == 1).count(), 1);
        assert_eq!(blocks[1024 / BLOCK], 1);
    }

    #[test]
    fn the_bench_sees_the_frames_a_cart_run_alone_shows() {
        let file = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../headless/tests/kept/dot.wasm"
        ))
        .unwrap();
        let instance = prepare(&file)
            .map_err(Stop::Refused)
            .and_then(|prepared| Wasmi::load(&prepared));
        let mut bench = Bench::start(instance, 1);
        let mut cart = Wasmi::open(&file, 1);
        let mut alone = Frames::default();
        for _ in 0..120 {
            bench.update(0);
            cart.update(0);
            alone.add(&cart);
        }
        assert_eq!(bench.frames(), alone);
        assert_ne!(alone, Frames::default());
    }

    #[test]
    fn a_cart_stopped_from_the_start_adds_no_frame() {
        let mut bench =
            Bench::<Wasmi>::start(Err(Stop::Refused(cartridge_run::Refusal::TooLarge(0))), 1);
        bench.update(0);
        bench.update(0);
        assert_eq!(bench.frames(), Frames::default());
    }

    #[test]
    fn a_stopped_cart_keeps_the_memory_it_stopped_with() {
        let trap = COUNTER.replace("i32.store)", "i32.store unreachable)");
        let mut bench = bench(&trap);
        bench.update(0);
        assert_eq!(bench.cart().stop(), Some(&Stop::Trap));
        assert_eq!(bench.memory()[1024], 1, "the store before the trap");
        bench.update(0);
        assert!(!bench.changed(1024));
    }
}

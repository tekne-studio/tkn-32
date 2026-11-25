//! A cart seen from inside, as the workbench sees it: every update's
//! buttons and the units it spent, the frames hash, and the whole memory
//! as the last run left it and as the run before left it. It sees by
//! reading, through the same [`Instance::read`] a runner reads the screen
//! with, so a game watched gets nothing a cabinet does not give it.

use cartridge::MAX_MEMORY;

use crate::{Frames, Instance};

/// A page of WebAssembly memory: how far a read reaches before the next.
const PAGE: usize = 64 * 1024;
/// The bytes one mark of the memory's map ([`Watch::blocks`]) stands for.
pub const BLOCK: usize = 256;

/// One update a watched cart ran: the buttons it was given and the meter's
/// units it spent, the screen and the palette read with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ran {
    pub buttons: u32,
    pub spent: u32,
}

/// What a watched cart ([`crate::Cart::watched`]) shows of itself.
#[derive(Debug, Default)]
pub struct Watch {
    memory: Vec<u8>,
    before: Vec<u8>,
    frames: Frames,
    ran: Vec<Ran>,
}

impl Watch {
    /// The memory as the last look read it.
    pub fn memory(&self) -> &[u8] {
        &self.memory
    }

    /// Whether the updates between the last two looks changed the byte at
    /// `at`, or made the memory reach it.
    pub fn changed(&self, at: usize) -> bool {
        self.memory.get(at) != self.before.get(at)
    }

    /// The memory's map: a byte a [`BLOCK`], 1 where the updates between
    /// the last two looks changed any byte of it.
    pub fn blocks(&self) -> Vec<u8> {
        (0..self.memory.len().div_ceil(BLOCK))
            .map(|block| {
                let at = block * BLOCK;
                u8::from(block_of(&self.memory, at) != block_of(&self.before, at))
            })
            .collect()
    }

    /// The frames hash of the updates run, the one the headless runner
    /// prints for the same seed and buttons.
    pub fn frames(&self) -> Frames {
        self.frames
    }

    /// Every update run, from the first: what a session keeps.
    pub fn ran(&self) -> &[Ran] {
        &self.ran
    }

    pub(crate) fn frames_mut(&mut self) -> &mut Frames {
        &mut self.frames
    }

    pub(crate) fn add(&mut self, ran: Ran) {
        self.ran.push(ran);
    }

    /// Reads the whole memory again, a page at a time until a read falls
    /// outside it, what was read before kept to compare with.
    pub(crate) fn look(&mut self, instance: &impl Instance) {
        std::mem::swap(&mut self.memory, &mut self.before);
        self.memory.clear();
        for page in 0..MAX_MEMORY / PAGE {
            let at = page * PAGE;
            self.memory.resize(at + PAGE, 0);
            if !instance.read(at as u32, &mut self.memory[at..]) {
                self.memory.truncate(at);
                break;
            }
        }
    }

    /// The first look: nothing before it changed anything.
    pub(crate) fn first(&mut self, instance: &impl Instance) {
        self.look(instance);
        self.before.clone_from(&self.memory);
    }
}

/// The block of `memory` from `at`, cut where the memory ends.
fn block_of(memory: &[u8], at: usize) -> Option<&[u8]> {
    memory.get(at..(at + BLOCK).min(memory.len()))
}

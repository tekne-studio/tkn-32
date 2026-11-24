//! The bench in the browser: a cartridge on the page's own WebAssembly
//! (`cartridge-web`'s `Browser`), as a cabinet runs it, behind the calls
//! the workbench's worker makes (`web/src/lib/bench.worker.ts`).

use cartridge::{PIXELS, UPDATE_BUDGET};
use cartridge_run::{Stop, prepare};
use cartridge_web::Browser;
use wasm_bindgen::prelude::*;

use crate::Bench;

/// One cartridge on the bench, for the worker.
#[wasm_bindgen]
pub struct Workbench {
    bench: Bench<Browser>,
}

#[wasm_bindgen]
impl Workbench {
    /// Prepares `file` and starts it with `seed`: running, or stopped with
    /// its screen saying why.
    #[wasm_bindgen(constructor)]
    pub fn new(file: &[u8], seed: u32) -> Workbench {
        let instance = prepare(file)
            .map_err(Stop::Refused)
            .and_then(|prepared| Browser::load(&prepared));
        Workbench {
            bench: Bench::start(instance, seed),
        }
    }

    /// One update with `buttons` held.
    pub fn update(&mut self, buttons: u32) {
        self.bench.update(buttons);
    }

    /// The screen in colour, four bytes a pixel, row by row.
    pub fn rgba(&self) -> Vec<u8> {
        let mut rgba = vec![0; PIXELS * 4];
        self.bench.cart().rgba(&mut rgba);
        rgba
    }

    /// The screen as indices into the palette, a byte a pixel.
    pub fn indices(&self) -> Vec<u8> {
        self.bench.cart().screen().to_vec()
    }

    /// The palette, four bytes a colour, as many colours as the cart holds.
    pub fn palette(&self) -> Vec<u8> {
        self.bench.cart().palette().concat()
    }

    /// The units the last update spent, out of [`budget`].
    pub fn spent(&self) -> u32 {
        self.bench.cart().spent()
    }

    pub fn updates(&self) -> f64 {
        self.bench.cart().updates() as f64
    }

    /// The frames hash so far, as `bun run cart` prints it.
    pub fn hash(&self) -> String {
        self.bench.frames().to_string()
    }

    /// Why it stopped, in the words its screen says, or none.
    pub fn stop(&self) -> Option<String> {
        self.bench.cart().stop().map(ToString::to_string)
    }

    /// The tones the last update played, seven words each in `env.tone`'s
    /// order.
    pub fn tones(&self) -> Vec<u32> {
        self.bench
            .tones()
            .iter()
            .flat_map(|tone| {
                [
                    u32::from(tone.channel),
                    tone.hz,
                    tone.ms,
                    u32::from(tone.wave),
                    tone.to_hz,
                    u32::from(tone.loud),
                    u32::from(tone.to_loud),
                ]
            })
            .collect()
    }

    /// How many bytes of memory it holds.
    pub fn size(&self) -> u32 {
        self.bench.memory().len() as u32
    }

    /// `len` bytes of memory from `at`, as many as lie inside it.
    pub fn memory(&self, at: u32, len: u32) -> Vec<u8> {
        let memory = self.bench.memory();
        let at = (at as usize).min(memory.len());
        memory[at..(at + len as usize).min(memory.len())].to_vec()
    }

    /// A byte for each of [`memory`]'s: 1 where the last update changed it.
    pub fn changes(&self, at: u32, len: u32) -> Vec<u8> {
        let at = at as usize;
        (at..at + len as usize)
            .take_while(|&byte| byte < self.bench.memory().len())
            .map(|byte| u8::from(self.bench.changed(byte)))
            .collect()
    }

    /// The memory's map: a byte a block of `crate::BLOCK` bytes, 1 where
    /// the last update changed it.
    pub fn blocks(&self) -> Vec<u8> {
        self.bench.blocks()
    }
}

/// The units an update may spend.
#[wasm_bindgen]
pub fn budget() -> u32 {
    UPDATE_BUDGET
}

/// The bytes one mark of the memory's map stands for.
#[wasm_bindgen]
pub fn block() -> u32 {
    crate::BLOCK as u32
}

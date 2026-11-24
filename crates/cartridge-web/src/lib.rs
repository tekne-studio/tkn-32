//! A cartridge in the browser: the page's own `WebAssembly` as an engine
//! behind `cartridge-run`'s [`Instance`](cartridge_run::Instance)
//! ([`Browser`]), and the worker that runs a page's cartridges beside its
//! frame, never on it.
//!
//! It is a WebAssembly file of its own, apart from the page's: a page that
//! runs no cartridge never fetches it, and a cartridge that
//! spends its whole budget every update slows the other cartridges, never
//! the frame. Natively it is empty: the native engine is wasmi, in
//! `cartridge-wasmi`.

#![cfg(target_arch = "wasm32")]

mod browser;

pub use browser::Browser;

use cartridge::{HEIGHT, PIXELS, WIDTH};
use cartridge_run::{Cart, Shelf, Stop, played_words, still};
use wasm_bindgen::prelude::*;

/// The cartridges a page's worker runs, a [`Shelf`] on the browser's own
/// WebAssembly, each by a name its host gives it. The worker's script is
/// the host's: it fetches each file, ticks the shelf and hands each screen
/// that changed to whatever draws it.
#[wasm_bindgen]
#[derive(Default)]
pub struct Carts {
    shelf: Shelf<Browser>,
}

#[wasm_bindgen]
impl Carts {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Carts {
        Carts::default()
    }

    /// Opens `file` as `id`, started with `seed`, its clock running from
    /// `now_ms`. A file the contract refuses, or one that stops as it
    /// starts, runs as its stop screen, which says why.
    pub fn open(&mut self, id: u32, file: &[u8], seed: u32, now_ms: f64) {
        self.shelf.put(id, Browser::open(file, seed), now_ms);
    }

    /// Puts `id` on the shelf as a cart whose file never came: its screen
    /// says so, its clock running from `now_ms`.
    pub fn fail(&mut self, id: u32, now_ms: f64) {
        self.shelf.put(id, Cart::stopped(Stop::Missing), now_ms);
    }

    /// Puts `id` on the shelf as the cover of `file`, named `name`, the
    /// hash of its bytes: still, never run (`cartridge_run::still`).
    pub fn cover(&mut self, id: u32, file: &[u8], name: &str, now_ms: f64) {
        self.shelf.put(id, still(file, name), now_ms);
    }

    pub fn close(&mut self, id: u32) {
        self.shelf.remove(id);
    }

    pub fn is_empty(&self) -> bool {
        self.shelf.is_empty()
    }

    /// Holds `buttons` down on `id` from its next update on.
    pub fn press(&mut self, id: u32, buttons: u32) {
        self.shelf.press(id, buttons);
    }

    /// Whether `id` stopped for good: its screen says why.
    pub fn stopped(&self, id: u32) -> bool {
        self.shelf.stopped(id)
    }

    /// Runs what is due by `now_ms`: the names of the carts whose screens
    /// changed.
    pub fn tick(&mut self, now_ms: f64) -> Vec<u32> {
        self.shelf.tick(now_ms)
    }

    /// The updates run since last asked and what each played, as words
    /// for the host (`cartridge_run::played_words`): an update that played
    /// nothing among them, so the host sounds each in its time.
    pub fn played(&mut self) -> Vec<u32> {
        played_words(&self.shelf.take_played())
    }

    /// The screen of `id` and its palette, as indices and colours
    /// (`cartridge_run::read_bytes`); empty when no cart is there by that
    /// name. What an agent reads of a game.
    pub fn read(&self, id: u32) -> Vec<u8> {
        self.shelf.read(id).unwrap_or_default()
    }

    /// The screen of `id` in colour, four bytes a pixel, [`width`] by
    /// [`height`]; empty when no cart is there by that name.
    pub fn rgba(&self, id: u32) -> Vec<u8> {
        let mut rgba = vec![0; PIXELS * 4];
        if !self.shelf.rgba(id, &mut rgba) {
            rgba.clear();
        }
        rgba
    }
}

/// A cartridge's screen, in pixels across and down.
#[wasm_bindgen]
pub fn width() -> u32 {
    WIDTH as u32
}

#[wasm_bindgen]
pub fn height() -> u32 {
    HEIGHT as u32
}

/// Whether `file` keeps the contract, as a host judges it before keeping
/// it: the code of why not ([`Refusal::code`](cartridge_run::Refusal::code)),
/// the one a host answers with, or empty when it does. What a page asks before
/// it sends a cartridge, so a refusal is told at once, and why.
#[wasm_bindgen]
pub fn check(file: &[u8]) -> String {
    cartridge_run::prepare(file)
        .err()
        .map_or_else(String::new, |refusal| refusal.code().to_owned())
}

/// What a page shows of a cartridge before it runs: its cover, read from
/// `file`, the bytes as they came, never by running it
/// ([`cartridge_run::cover`]). `name` is the file's, the hash of its
/// bytes: a file with no cover shows the start of it.
#[wasm_bindgen]
pub fn cover(file: &[u8], name: &str) -> Covered {
    let cover = cartridge_run::cover(file);
    Covered {
        said: cover.map(|cover| (cover.title().into(), cover.maker().into(), cover.year())),
        rgba: cartridge_run::cover_rgba(cover.as_ref(), name),
    }
}

/// A cartridge's cover as a page reads it: its words, when its file has a
/// cover, and its picture in colour, the host's stand-in when it has none.
#[wasm_bindgen]
pub struct Covered {
    said: Option<(String, String, u16)>,
    rgba: Vec<u8>,
}

#[wasm_bindgen]
impl Covered {
    /// Its title; none for a file with no cover.
    pub fn title(&self) -> Option<String> {
        self.said.as_ref().map(|(title, _, _)| title.clone())
    }

    /// Who made it, as plain text: `TEKNE STUDIO`.
    pub fn maker(&self) -> Option<String> {
        self.said.as_ref().map(|(_, maker, _)| maker.clone())
    }

    pub fn year(&self) -> Option<u16> {
        self.said.as_ref().map(|&(_, _, year)| year)
    }

    /// The picture, four bytes a pixel, [`width`] by [`height`].
    pub fn rgba(&self) -> Vec<u8> {
        self.rgba.clone()
    }
}

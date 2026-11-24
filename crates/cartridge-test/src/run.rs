//! What a run of a cartridge comes to, on any engine and through any host:
//! the hash of every frame it showed and of every tone it played, why it
//! stopped, the updates it ran, its last screen in the colours a host
//! draws, and what a host's check says of its file.

use std::fmt;

use cartridge::PIXELS;
use cartridge_run::{Cart, Instance, Stop};
use cartridge_wasmi::Wasmi;

/// What a run came to: the hash of every frame shown after an update, the
/// hash of every tone kept, in `env.tone`'s words, why it stopped, if it
/// did, the updates it ran, the hash of its last screen in colour, and the
/// verdict on its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub hash: u32,
    pub tones: u32,
    /// `running`, or why it stopped, as its screen says it.
    pub stop: String,
    pub updates: u64,
    pub rgba: u32,
    /// The code of why a host's check refuses the file, as a host judges
    /// one before keeping it; empty when it keeps it. A host refuses to
    /// open a file for the same reason, so a run refused is refused here.
    pub check: String,
}

impl fmt::Display for Run {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:08x} {:08x} {} {} {:08x} check:{}",
            self.hash, self.tones, self.stop, self.updates, self.rgba, self.check
        )
    }
}

/// FNV-1a over 32 bits, as `run.mjs` hashes.
pub fn fnv(hash: u32, bytes: &[u8]) -> u32 {
    bytes.iter().fold(hash, |hash, &byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// `cart` run with `buttons[i]` held at update `i`, until it stops.
pub fn run<I: Instance>(mut cart: Cart<I>, buttons: &[u32]) -> Run {
    let mut hash = 0x811c_9dc5;
    let mut tones = 0x811c_9dc5;
    for &held in buttons {
        if cart.stop().is_some() {
            break;
        }
        for tone in cart.update(held) {
            let words = [
                u32::from(tone.channel),
                tone.hz,
                tone.ms,
                u32::from(tone.wave),
                tone.to_hz,
                u32::from(tone.loud),
                u32::from(tone.to_loud),
            ];
            for word in words {
                tones = fnv(tones, &word.to_le_bytes());
            }
        }
        if cart.stop().is_none() {
            hash = fnv(hash, cart.screen());
            hash = fnv(hash, cart.palette().as_flattened());
        }
    }
    let mut rgba = vec![0; PIXELS * 4];
    cart.rgba(&mut rgba);
    let stop = cart.stop();
    Run {
        hash,
        tones,
        stop: stop.map_or_else(|| "running".to_owned(), ToString::to_string),
        updates: cart.updates(),
        rgba: fnv(0x811c_9dc5, &rgba),
        check: match stop {
            Some(Stop::Refused(refusal)) => refusal.code().to_owned(),
            _ => String::new(),
        },
    }
}

/// `file` run on wasmi from `seed`, `buttons[i]` held at update `i`.
pub fn on_wasmi(file: &[u8], seed: u32, buttons: &[u32]) -> Run {
    run(Wasmi::open(file, seed), buttons)
}

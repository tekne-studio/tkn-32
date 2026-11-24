//! The chip as an AudioWorklet runs it: plain exports over one chip, no
//! bindings, because a worklet's scope lacks what wasm-bindgen's glue
//! reaches for, and its processor (`worklet.js`, beside this crate's
//! manifest, served with the module by every page that sounds a cart)
//! needs only three calls: start at its rate, play a tone, render a block.

use std::cell::RefCell;

use chip::Chip;

/// The most samples one [`render`] gives: an AudioWorklet's block.
const BLOCK: usize = 128;

struct Worklet {
    chip: Chip,
    out: [f32; BLOCK],
}

thread_local! {
    static WORKLET: RefCell<Option<Worklet>> = const { RefCell::new(None) };
}

/// A silent chip at `rate` samples a second, in place of any before.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn start(rate: f32) {
    WORKLET.set(Some(Worklet {
        chip: Chip::new(rate),
        out: [0.0; BLOCK],
    }));
}

/// A tone in `env.tone`'s words ([`Chip::play`]).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn play(
    channel: u32,
    hz: u32,
    ms: u32,
    wave: u32,
    to_hz: u32,
    loud: u32,
    to_loud: u32,
) {
    WORKLET.with_borrow_mut(|worklet| {
        if let Some(worklet) = worklet {
            worklet
                .chip
                .play(channel, hz, ms, wave, to_hz, loud, to_loud);
        }
    });
}

/// Renders the next `len` samples, at most a block, and says where they
/// are in memory; null before [`start`]. They stay there until the next
/// render: the worklet copies them out first.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn render(len: u32) -> *const f32 {
    WORKLET.with_borrow_mut(|worklet| match worklet {
        Some(worklet) => {
            let len = (len as usize).min(BLOCK);
            worklet.chip.render(&mut worklet.out[..len]);
            worklet.out.as_ptr()
        }
        None => std::ptr::null(),
    })
}

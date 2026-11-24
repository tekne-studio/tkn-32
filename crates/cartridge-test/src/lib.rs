//! What a cartridge's tests run it with, a dev-dependency of a cart: its
//! own crate built to WebAssembly as a release builds it ([`built!`]), and
//! a run of a file on wasmi and on every browser engine at hand, which must
//! agree to the frame ([`on_every_engine`]). The runner's own suites take
//! [`Run`] from here too.

#[cfg(not(target_arch = "wasm32"))]
mod built;
#[cfg(not(target_arch = "wasm32"))]
mod engines;
mod run;

#[cfg(not(target_arch = "wasm32"))]
pub use built::built;
#[cfg(not(target_arch = "wasm32"))]
pub use engines::on_every_engine;
pub use run::{Run, on_wasmi, run};

/// The `.wasm` of the crate whose test calls it, built once for the test
/// binary in release for `wasm32-unknown-unknown`, in the cargo workspace
/// it is a member of.
#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! built {
    () => {
        $crate::built(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"))
    };
}

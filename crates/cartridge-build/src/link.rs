//! The WebAssembly link of a cart: its stack, and a map of where each of
//! its statics lies, which the workbench names the memory by.

/// The file in the build's `OUT_DIR` the linker writes its map to: each
/// static's address, size and symbol. Writing it changes no byte of the
/// cart.
pub const MAP: &str = "link.map";

/// Links the cart whose build script calls it with a stack of `stack`
/// bytes, and its map to [`MAP`]. A cartridge has a megabyte of memory and
/// the linker's default stack is a megabyte alone, so a cart that does not
/// call it starts its data past the megabyte and is refused. Natively, as
/// in a test, it does nothing.
pub fn link(stack: u32) {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    let out = std::env::var("OUT_DIR").expect("run from a build script");
    println!("cargo::rustc-link-arg-cdylib=-zstack-size={stack}");
    println!("cargo::rustc-link-arg-cdylib=--Map={out}/{MAP}");
}

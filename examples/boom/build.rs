// A stack of 64 KiB, and a map of the statics for the workbench.
fn main() {
    cartridge_build::link(64 * 1024);
}

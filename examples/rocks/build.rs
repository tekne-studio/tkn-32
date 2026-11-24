// A stack of 64 KiB. Its cover, drawn by the game itself (tests/cover.rs),
// goes into its file.
fn main() {
    cartridge_build::link(64 * 1024);
    cartridge_build::cover("cover.png", "ROCKS", "TEKNE STUDIO", 2026);
}

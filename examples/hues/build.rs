// A stack of 64 KiB, and a map of the statics for the workbench. Its
// cover, painted in all 32 colours, goes into its file.
fn main() {
    cartridge_build::link(64 * 1024);
    cartridge_build::cover("cover.png", "HUES", "TEKNE STUDIO", 2026);
}

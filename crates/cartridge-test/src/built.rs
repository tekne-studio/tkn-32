//! A cart crate built to WebAssembly as a release builds it, in the cargo
//! workspace it is a member of, for its own tests.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// The `.wasm` of the cart crate `package`, whose manifest is in `dir`,
/// built once for the test binary, in a target folder of its own so the
/// cargo running the tests is not waited on. [`built!`](crate::built!)
/// calls it with the calling crate's own.
pub fn built(dir: &str, package: &str) -> &'static [u8] {
    static FILES: OnceLock<Mutex<HashMap<String, &'static [u8]>>> = OnceLock::new();
    let mut files = FILES
        .get_or_init(Mutex::default)
        .lock()
        .expect("no build panicked");
    files
        .entry(package.to_owned())
        .or_insert_with(|| Vec::leak(build(Path::new(dir), package)))
}

fn build(dir: &Path, package: &str) -> Vec<u8> {
    let root = dir
        .ancestors()
        .find(|dir| dir.join("Cargo.lock").exists())
        .expect("the cart is in a cargo workspace");
    let target = root.join("target/carts");
    let status = Command::new(env!("CARGO"))
        .current_dir(root)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .args(["-p", package, "--target-dir"])
        .arg(&target)
        .status()
        .expect("cargo runs");
    assert!(
        status.success(),
        "{package} builds to wasm32-unknown-unknown"
    );
    // Cargo names a library's file by its crate's name: `-` becomes `_`.
    let file = format!(
        "wasm32-unknown-unknown/release/{}.wasm",
        package.replace('-', "_")
    );
    std::fs::read(target.join(file)).expect("the .wasm is where cargo puts it")
}

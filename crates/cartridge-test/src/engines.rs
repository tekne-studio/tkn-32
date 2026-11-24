//! A run of the same prepared bytes on wasmi and on each browser engine at
//! hand, node for V8 and bun for JavaScriptCore, through `cart.mjs`, which
//! holds a cartridge to the contract as `Cart` does.

use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use cartridge::{
    ABI, CHANNELS, COLOURS, Export, LOUDEST, MAX_TONES, MEMORY, PIXELS, SCREENS, TONE,
    UPDATE_BUDGET, button, wave,
};
use cartridge_run::{METER, OUT_OF_MEMORY, START, TOO_DEEP, prepare};

use crate::run::{Run, on_wasmi};

/// The browser's engines, by the command that runs each.
const ENGINES: [&str; 2] = ["node", "bun"];

/// Read into the test, so it is found from whichever crate includes this.
const SCRIPT: &str = include_str!("cart.mjs");

/// `file` run on wasmi and on every browser engine at hand, which must
/// agree to the frame: the same frames, the same stop at the same update.
/// Gives what they agree on. A file `prepare` refuses runs on wasmi alone.
pub fn on_every_engine(file: &[u8], seed: u32, buttons: &[u32]) -> Run {
    let wasmi = on_wasmi(file, seed, buttons);
    let Ok(prepared) = prepare(file) else {
        return wasmi;
    };
    // The system's temporary folder: a library is given no target folder
    // of its own, and the process and a count keep each run's files apart.
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir();
    let name = format!(
        "cartridge-test-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    );
    let [script, wasm, held] =
        ["mjs", "wasm", "buttons"].map(|kind| dir.join(format!("{name}.{kind}")));
    let bytes: Vec<u8> = buttons.iter().flat_map(|b| b.to_le_bytes()).collect();
    for (path, bytes) in [
        (&script, SCRIPT.as_bytes()),
        (&wasm, prepared.bytes()),
        (&held, &bytes),
    ] {
        std::fs::write(path, bytes).expect("the temporary folder takes a file");
    }

    let names = |exports: &[Export]| {
        exports
            .iter()
            .map(|export| format!(r#""{}""#, export.name()))
            .collect::<Vec<_>>()
            .join(",")
    };
    let (exports, optional) = (names(&Export::REQUIRED), names(&Export::OPTIONAL));
    let contract = format!(
        r#"{{"abi":{ABI},"screens":{SCREENS},"updateBudget":{UPDATE_BUDGET},"pixels":{PIXELS},"colours":{COLOURS},"buttons":{},"outOfMemory":{OUT_OF_MEMORY},"tooDeep":{TOO_DEEP},"tone":["{}","{}"],"channels":{CHANNELS},"waves":{},"maxTones":{MAX_TONES},"loudest":{LOUDEST},"exports":[{exports}],"optional":[{optional}],"memory":"{MEMORY}","meter":"{METER}","start":"{START}"}}"#,
        button::ALL,
        TONE[0],
        TONE[1],
        wave::COUNT,
    );
    let mut ran = 0;
    for engine in ENGINES {
        let Ok(output) = Command::new(engine)
            .arg(&script)
            .arg(&wasm)
            .arg(seed.to_string())
            .arg(&held)
            .arg(&contract)
            .output()
        else {
            continue;
        };
        assert!(
            output.status.success(),
            "{engine}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let line = String::from_utf8_lossy(&output.stdout);
        assert_eq!(line.trim(), wasmi.to_string(), "{engine} against wasmi");
        ran += 1;
    }
    assert!(
        ran > 0,
        "no browser engine to compare: node (V8) or bun (JavaScriptCore) on the PATH runs these tests"
    );
    for path in [script, wasm, held] {
        let _ = std::fs::remove_file(path);
    }
    wasmi
}

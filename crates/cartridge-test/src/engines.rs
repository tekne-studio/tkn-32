//! A run of the same file on wasmi, as the headless runner runs it, and
//! through the emulator's runner by its JavaScript host, `run.mjs`, on
//! every engine at hand: V8 (node), JavaScriptCore (bun) and SpiderMonkey
//! (its shell, `bun run setup`), each compiler of V8 and SpiderMonkey
//! forced alone as well as tiering as a browser does. A native host holds
//! itself to the same runs through the emulator's cases ([`keep_case`]).
//!
//! The emulator is the folder `EMULATOR` names: a build of the emulator
//! names its own, to keep its cases, and a game the one it ships on, a
//! host's pin, in its `.cargo/config.toml`. Else it is the first found up
//! from the crate under test, then up from this console: an `emulator/` a
//! game keeps, or a console's own build (`target/emulator`, `bun run
//! emulator`), which an example in the console runs.

use std::path::{Path, PathBuf};

use crate::run::{Run, fnv, on_wasmi};
use std::io::Write;
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// A JavaScript engine the host runs on: a program, the flags that pick
/// its compilers, and what comes before `run.mjs`'s arguments.
struct Engine {
    name: &'static str,
    program: Program,
    flags: &'static [&'static str],
}

/// Where an engine's program is.
enum Program {
    /// A command on the PATH.
    Path(&'static str),
    /// SpiderMonkey's shell, where `bun run setup` puts it.
    SpiderMonkey,
}

/// The engines every run goes through. A browser's engine compiles a
/// module at once with its baseline compiler and moves what runs often to
/// its optimising one: each is forced alone here as well, so a difference
/// between them is caught whichever a browser happens to run.
const ENGINES: [Engine; 7] = [
    Engine {
        name: "V8, tiering",
        program: Program::Path("node"),
        flags: &[],
    },
    Engine {
        name: "V8, Liftoff alone",
        program: Program::Path("node"),
        flags: &["--liftoff", "--no-wasm-tier-up"],
    },
    Engine {
        name: "V8, TurboFan alone",
        program: Program::Path("node"),
        flags: &["--no-liftoff"],
    },
    Engine {
        name: "JavaScriptCore",
        program: Program::Path("bun"),
        flags: &[],
    },
    Engine {
        name: "SpiderMonkey, tiering",
        program: Program::SpiderMonkey,
        flags: &[],
    },
    Engine {
        name: "SpiderMonkey, baseline alone",
        program: Program::SpiderMonkey,
        flags: &["--wasm-compiler=baseline"],
    },
    Engine {
        name: "SpiderMonkey, Ion alone",
        program: Program::SpiderMonkey,
        flags: &["--wasm-compiler=optimizing"],
    },
];

/// SpiderMonkey's shell, `.tools/spidermonkey/js` in the console's folder,
/// an ancestor of the crate under test; `None`, said once on the terminal
/// past the tests' capture, when `bun run setup` has not put it there.
fn spidermonkey() -> Option<PathBuf> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let tested = std::env::var("CARGO_MANIFEST_DIR").expect("cargo runs the test");
            let found = Path::new(&tested)
                .ancestors()
                .map(|dir| dir.join(".tools/spidermonkey/js"))
                .find(|js| js.exists());
            if found.is_none() {
                // Straight to the terminal: the tests capture what they print.
                let _ = writeln!(
                    std::io::stderr(),
                    "\n!!! SpiderMonkey's shell is not here: Firefox's engine is SKIPPED. `bun run setup` in tkn-32 downloads it. !!!\n"
                );
            }
            found
        })
        .clone()
}

/// Written beside each run, so a JavaScript engine finds it by a path.
const SCRIPT: &str = include_str!("run.mjs");

/// The emulator the tests run: the folder `EMULATOR` names, a build's of
/// the emulator keeping its cases ([`keep_case`]) or a game's, else the
/// first up from the folder cargo names the crate under test by as its
/// tests run, then up from this crate's own, the console's checkout.
pub fn emulator() -> PathBuf {
    if let Some(folder) = std::env::var_os("EMULATOR") {
        return PathBuf::from(folder);
    }
    let tested = std::env::var("CARGO_MANIFEST_DIR").expect("cargo runs the test");
    Path::new(&tested)
        .ancestors()
        .chain(Path::new(env!("CARGO_MANIFEST_DIR")).ancestors())
        .flat_map(|dir| [dir.join("emulator"), dir.join("target/emulator")])
        .find(|folder| folder.join("emulator.json").exists())
        .expect("no emulator built: `bun run emulator` in tkn-32 builds it")
}

/// The file the emulator's manifest names for `part`: `runner`, `host`,
/// `chip` or `processor`.
pub fn part(folder: &Path, part: &str) -> PathBuf {
    let manifest = std::fs::read_to_string(folder.join("emulator.json"))
        .expect("the emulator's manifest reads");
    let key = format!("\"{part}\"");
    let after = manifest
        .find(&key)
        .map(|at| &manifest[at + key.len()..])
        .unwrap_or_else(|| panic!("the manifest names no {part}"));
    let name = after.split('"').nth(1).expect("a name in quotes");
    folder.join(name)
}

/// `file` run on wasmi and through every host at hand, which must agree to
/// the frame: the same frames, the same tones, the same stop at the same
/// update, and the same verdict of their check. Gives what they agree on.
pub fn on_every_engine(file: &[u8], seed: u32, buttons: &[u32]) -> Run {
    let wasmi = on_wasmi(file, seed, buttons);
    let emulator = emulator();
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
    for (path, bytes) in [(&script, SCRIPT.as_bytes()), (&wasm, file), (&held, &bytes)] {
        std::fs::write(path, bytes).expect("the temporary folder takes a file");
    }
    let mut ran = 0;
    for engine in &ENGINES {
        let mut command = match engine.program {
            Program::Path(program) => {
                let mut command = Command::new(program);
                command.args(engine.flags).arg(&script);
                command
            }
            Program::SpiderMonkey => {
                let Some(js) = spidermonkey() else { continue };
                let mut command = Command::new(js);
                command.args(engine.flags).arg("-m").arg(&script).arg("--");
                command
            }
        };
        let Ok(output) = command
            .arg(&emulator)
            .arg(&wasm)
            .arg(seed.to_string())
            .arg(&held)
            .output()
        else {
            continue;
        };
        let name = engine.name;
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let line = String::from_utf8_lossy(&output.stdout);
        assert_eq!(line.trim(), wasmi.to_string(), "{name} against wasmi");
        ran += 1;
    }
    assert!(
        ran > 0,
        "no JavaScript engine to run the emulator's host: node (V8) or bun (JavaScriptCore) on the PATH runs these tests"
    );
    for path in [script, wasm, held] {
        let _ = std::fs::remove_file(path);
    }
    if let Some(folder) = std::env::var_os("CASES") {
        keep_case(Path::new(&folder), file, seed, buttons, &wasmi);
    }
    wasmi
}

/// Keeps a case every host agreed on in `folder`, a file each, for the
/// emulator's build to gather into its cases (`bun run emulator`): the
/// test it came from, the cart's file, its seed, its buttons, a byte an
/// update, as runs of one value, what every host's check says of the file,
/// and the run every host must give. A file past the most a cartridge may
/// have is no case: its bytes would outweigh every other case's, and a
/// host makes its own from the manifest's `most`.
fn keep_case(folder: &Path, file: &[u8], seed: u32, buttons: &[u32], run: &Run) {
    if file.len() > cartridge::MAX_FILE {
        return;
    }
    let test = std::thread::current().name().unwrap_or("").to_owned();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for &held in buttons {
        match runs.last_mut() {
            Some((value, count)) if *value == held => *count += 1,
            _ => runs.push((held, 1)),
        }
    }
    let buttons = runs
        .iter()
        .map(|(value, count)| format!("[{value},{count}]"))
        .collect::<Vec<_>>()
        .join(",");
    let hex: String = file.iter().map(|byte| format!("{byte:02x}")).collect();
    let stop = quoted(&run.stop);
    let case = format!(
        r#"{{"test":"{test}","seed":{seed},"file":"{hex}","buttons":[{buttons}],"check":"{}","run":{{"frames":"{:08x}","tones":"{:08x}","stop":"{stop}","updates":{},"rgba":"{:08x}"}}}}"#,
        run.check, run.hash, run.tones, run.updates, run.rgba
    );
    let name = format!("{:08x}.json", fnv(0x811c_9dc5, case.as_bytes()));
    std::fs::create_dir_all(folder).expect("the cases' folder is made");
    std::fs::write(folder.join(name), case).expect("the cases' folder takes a case");
}

/// `text` inside a JSON string: a refusal's words may carry a line break.
fn quoted(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '"' | '\\' => format!("\\{c}"),
            c if c < ' ' => format!("\\u{:04x}", u32::from(c)),
            c => c.to_string(),
        })
        .collect()
}

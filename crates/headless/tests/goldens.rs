//! The tones of the arcade's own carts, Rocks and the guide's examples,
//! heard through the chip as `bun run cart --wav` hears them, held to a
//! hash each: what changes how a cartridge sounds anywhere the chip plays
//! it changes one of these on purpose.
//!
//! The carts are kept as they were built (`tests/kept/`), so a game that
//! changes its own sound moves nothing here; each runs a session of buttons
//! from its seed, and its frames hash says the kept file still runs that
//! game. A file kept again, rebuilt from its folder, gives new hashes; Dot's
//! never is, since the contract holds it as a file already made
//! (`cartridge-wasmi`'s `a_kept_file_runs_as_it_always_has`).
//!
//! What each session played is kept beside its cart
//! (`kept/<name>.played.json`): the tones an update, the samples' hash and
//! how many there are, for the worklet's test
//! (`crates/chip-worklet/worklet.test.ts`), which plays them as a page
//! feeds it and holds what it renders to the same hash. A session that
//! moves writes them anew with `WRITE_PLAYED=1 cargo test -p headless
//! --test goldens`.

use cartridge_run::{Frames, WORDS, prepare};
use cartridge_wasmi::Wasmi;
use headless::{Heard, Script};

/// A kept cart, the session it plays and what it shows and sounds.
struct Golden {
    name: &'static str,
    file: &'static [u8],
    seed: u32,
    buttons: &'static str,
    updates: u32,
    frames: &'static str,
    samples: u64,
}

const GOLDENS: [Golden; 4] = [
    // A game: shots, thrust, rocks breaking, a ship lost.
    Golden {
        name: "rocks",
        file: include_bytes!("kept/rocks.wasm"),
        seed: 1,
        buttons: "30:start 34:- 60:a+right 90:up 120:a 124:- 128:a 132:up+a+left 200:- \
                  210:a 214:- 218:a 222:- 300:up+right 360:a 364:- 400:b 404:- 500:a+left \
                  600:up+a 700:-",
        updates: 900,
        frames: "298e562d7bf5b62a",
        samples: 0xCE36_3D90_5FCE_0883,
    },
    // A beep a press, one replacing the next three updates on.
    Golden {
        name: "dot",
        file: include_bytes!("kept/dot.wasm"),
        seed: 1,
        buttons: "10:right 20:a 23:- 26:a 29:- 60:a 64:- 120:a+down 124:-",
        updates: 240,
        frames: "ec920574cb216309",
        samples: 0x7D15_4DFC_58F0_35EB,
    },
    // The boom, the whoosh over it, and each struck again while it plays.
    Golden {
        name: "boom",
        file: include_bytes!("kept/boom.wasm"),
        seed: 1,
        buttons: "10:a 14:- 20:b 24:- 30:a 34:- 60:a+b 64:- 90:b 94:- 96:b 100:-",
        updates: 240,
        frames: "fe39a167a1efbd0a",
        samples: 0x77F7_6ABA_1C68_0E6C,
    },
    // A triangle a colour, up the bands.
    Golden {
        name: "hues",
        file: include_bytes!("kept/hues.wasm"),
        seed: 1,
        buttons: "10:a 14:- 20:right 24:- 30:a 34:- 40:right 44:- 50:a 54:- 56:a 60:-",
        updates: 240,
        frames: "eebdee8f0ec912f5",
        samples: 0x3445_A012_551D_C748,
    },
];

/// FNV-1a over the samples' bits.
fn fingerprint(samples: &[f32]) -> u64 {
    samples.iter().fold(0xCBF2_9CE4_8422_2325, |hash, sample| {
        sample
            .to_bits()
            .to_le_bytes()
            .iter()
            .fold(hash, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01B3)
            })
    })
}

/// What a golden's session gives: its frames hash, its samples' hash and
/// how many samples there are, and the words of each update's tones.
struct Session {
    frames: String,
    samples: u64,
    length: usize,
    updates: Vec<Vec<u32>>,
}

impl Session {
    /// The kept file: one update a line, so a moved tone is one line in a
    /// diff. The hash is a string, past what a JavaScript number holds.
    fn played(&self) -> String {
        let updates: Vec<String> = self
            .updates
            .iter()
            .map(|words| {
                let words: Vec<String> = words.iter().map(u32::to_string).collect();
                format!("[{}]", words.join(","))
            })
            .collect();
        format!(
            "{{\n\"samples\": \"{:#018X}\",\n\"length\": {},\n\"updates\": [\n{}\n]\n}}\n",
            self.samples,
            self.length,
            updates.join(",\n")
        )
    }
}

/// Runs a golden's session.
fn run(golden: &Golden) -> Session {
    let prepared = prepare(golden.file).expect("a kept cart is prepared");
    let mut cart = Wasmi::cart(&prepared, golden.seed);
    let script = Script::parse(golden.buttons).expect("a session reads");
    let mut frames = Frames::default();
    let mut heard = Heard::default();
    let mut updates = Vec::new();
    for tick in 0..golden.updates {
        let played = cart.update(script.held(tick));
        heard.update(played);
        updates.push(played.iter().flat_map(|tone| tone.words()).collect());
        frames.add(&cart);
        assert!(cart.stop().is_none(), "{}: stopped at {tick}", golden.name);
    }
    let samples = heard.end();
    Session {
        frames: frames.to_string(),
        samples: fingerprint(&samples),
        length: samples.len(),
        updates,
    }
}

#[test]
fn each_game_sounds_as_it_always_did() {
    let mut moved = Vec::new();
    for golden in &GOLDENS {
        let session = run(golden);
        let tones = session.updates.iter().flatten().count() / WORDS;
        assert!(
            tones >= 3,
            "{}: a session that sounds, {tones} tones",
            golden.name
        );
        if (session.frames.as_str(), session.samples) != (golden.frames, golden.samples) {
            moved.push(format!(
                "{}: frames {}, samples {:#018X}",
                golden.name, session.frames, session.samples
            ));
        }
    }
    assert!(moved.is_empty(), "moved:\n{}", moved.join("\n"));
}

#[test]
fn what_each_session_played_is_kept_for_the_worklet() {
    let write = std::env::var_os("WRITE_PLAYED").is_some();
    for golden in &GOLDENS {
        let path = format!(
            "{}/tests/kept/{}.played.json",
            env!("CARGO_MANIFEST_DIR"),
            golden.name
        );
        let played = run(golden).played();
        if write {
            std::fs::write(&path, &played).expect("the kept file is written");
        }
        let kept = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            kept == played,
            "{}: {path} is not what the session plays; WRITE_PLAYED=1 writes it",
            golden.name
        );
    }
}

//! A cartridge run with no window, on wasmi, as a cabinet runs one: a seed
//! and a script of buttons in; pictures of its screen, its tones heard
//! through the arcade's chip, the frames hash and what it spent out. The
//! same seed and buttons give the same frames hash the workbench shows, so
//! a session saved there is played again here exactly.
//!
//! ```text
//! headless FILE.wasm --frames N [--seed N] [--buttons SCRIPT|@FILE]
//!          [--out DIR [--every N] [--at N,N...] [--scale N]] [--wav FILE]
//! ```
//!
//! `bun run cart` builds the cart named and runs it here.

mod out;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use cartridge::{PIXELS, UPDATE_BUDGET};
use cartridge_run::{Cart, Frames, Stop, prepare};
use cartridge_wasmi::Wasmi;
use headless::{Heard, RATE, Script};

use out::{Picture, save_wav};

struct Run {
    file: PathBuf,
    seed: u32,
    buttons: Script,
    frames: u32,
    /// Where the pictures go: one PNG a saved frame, and `strip.png`.
    out: Option<PathBuf>,
    /// Save one frame every this many updates, and these, by the updates
    /// run before them.
    every: u32,
    at: Vec<u32>,
    /// Screen pixels a picture's pixel is wide.
    scale: u32,
    /// Where the tones go, heard through the chip.
    wav: Option<PathBuf>,
}

fn main() -> ExitCode {
    match parse(std::env::args().skip(1)).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("headless: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(run: Run) -> Result<(), String> {
    let file = std::fs::read(&run.file).map_err(|e| format!("{}: {e}", run.file.display()))?;
    if let Some(out) = &run.out {
        std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    let preparing = Instant::now();
    let prepared = prepare(&file);
    let took_preparing = preparing.elapsed();
    match &prepared {
        Ok(prepared) => println!(
            "{}: {} bytes, {} prepared in {}",
            run.file.display(),
            file.len(),
            prepared.bytes().len(),
            micros(took_preparing)
        ),
        Err(refusal) => println!(
            "{}: {} bytes, refused: {refusal}",
            run.file.display(),
            file.len()
        ),
    }

    let opening = Instant::now();
    let mut cart = match prepared {
        Ok(prepared) => Wasmi::cart(&prepared, run.seed),
        Err(refusal) => Cart::stopped(Stop::Refused(refusal)),
    };
    let opened = opening.elapsed();
    let mut frames = Frames::default();
    let mut heard = run.wav.as_ref().map(|_| Heard::default());
    let mut took = Vec::with_capacity(run.frames as usize);
    let (mut spent, mut peak, mut tones) = (0_u64, 0, 0);
    let mut rgba = vec![0; PIXELS * 4];
    let mut saved = Vec::new();
    for tick in 0..run.frames {
        // A cart stopped from the start runs nothing, as on the workbench.
        if cart.stop().is_some() {
            break;
        }
        let updating = Instant::now();
        let played = cart.update(run.buttons.held(tick));
        took.push(updating.elapsed());
        tones += played.len();
        if let Some(heard) = &mut heard {
            heard.update(played);
        }
        let stopped = cart.stop().is_some();
        if !stopped {
            spent += u64::from(cart.spent());
            peak = peak.max(cart.spent());
        }
        frames.add(&cart);
        let updates = tick + 1;
        if let Some(out) = &run.out
            && (updates % run.every == 0
                || run.at.contains(&updates)
                || stopped
                || updates == run.frames)
        {
            cart.rgba(&mut rgba);
            let picture = Picture::of_screen(&rgba, run.scale);
            picture.save(&out.join(format!("{updates:05}.png")))?;
            saved.push(picture);
        }
        if stopped {
            break;
        }
    }

    let ran = took.len() as u32;
    println!(
        "seed {}, {ran} updates, frames hash {frames}, opened in {}",
        run.seed,
        micros(opened)
    );
    if ran > 0 {
        took.sort_unstable();
        let mean = took.iter().sum::<Duration>() / ran;
        println!(
            "update on wasmi: mean {}, median {}, slowest {}",
            micros(mean),
            micros(took[took.len() / 2]),
            micros(took[took.len() - 1])
        );
        println!(
            "meter: mean {} units, peak {peak} of {UPDATE_BUDGET}; {tones} tones",
            spent / u64::from(ran)
        );
    }
    if let Some(stop) = cart.stop() {
        println!("stopped after {} updates: {stop}", cart.updates());
    }
    if let Some(out) = &run.out {
        let strip = out.join("strip.png");
        Picture::strip(&saved, run.scale).save(&strip)?;
        println!(
            "{} frames in {}, all in {}",
            saved.len(),
            out.display(),
            strip.display()
        );
    }
    if let (Some(wav), Some(heard)) = (&run.wav, heard) {
        let samples = heard.end();
        save_wav(wav, &samples, RATE)?;
        let seconds = samples.len() as f32 / RATE as f32;
        println!("{seconds:.1} s of tones in {}", wav.display());
    }
    Ok(())
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Run, String> {
    let file = args
        .next()
        .filter(|file| !file.starts_with("--"))
        .ok_or("headless needs the cartridge's .wasm first")?;
    let mut run = Run {
        file: PathBuf::from(file),
        seed: 1,
        buttons: Script::default(),
        frames: 0,
        out: None,
        every: 60,
        at: Vec::new(),
        scale: 4,
        wav: None,
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--seed" => run.seed = number(&value()?, "--seed")?,
            "--buttons" => {
                let text = value()?;
                let script = match text.strip_prefix('@') {
                    Some(path) => {
                        std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?
                    }
                    None => text,
                };
                run.buttons = Script::parse(&script)?;
            }
            "--frames" => run.frames = number(&value()?, "--frames")?,
            "--out" => run.out = Some(PathBuf::from(value()?)),
            "--every" => run.every = number::<u32>(&value()?, "--every")?.max(1),
            "--at" => {
                for tick in value()?.split(',').filter(|tick| !tick.is_empty()) {
                    run.at.push(number(tick, "--at")?);
                }
            }
            "--scale" => run.scale = number::<u32>(&value()?, "--scale")?.clamp(1, 16),
            "--wav" => run.wav = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if run.frames == 0 {
        return Err("headless runs --frames N updates, N at least 1".into());
    }
    Ok(run)
}

fn number<T: std::str::FromStr>(text: &str, flag: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{flag}: cannot read {text:?} as a number"))
}

fn micros(duration: Duration) -> String {
    format!("{:.1} µs", duration.as_secs_f64() * 1e6)
}

/// Single-threaded profiling target for coz.
///
/// Usage:
///   cargo build --release --example profile
///   coz run --- ./target/release/examples/profile <ROM> [headless|rendering] [SECONDS]

use std::env;
use std::fs;
use std::time::{Duration, Instant};

use stella_rs::atari::{Action, Atari, HeadlessAtari};

fn main() {
    coz::thread_init();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ROM> [headless|rendering] [SECONDS]", args[0]);
        std::process::exit(1);
    }

    let rom = fs::read(&args[1]).expect("failed to read ROM file");
    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("headless");
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    let actions = [Action::Noop, Action::Fire, Action::Left, Action::Right];
    let deadline = Instant::now() + Duration::from_secs(duration_secs);
    let mut frames: u64 = 0;
    let mut cycles: u64 = 0;

    match mode {
        "headless" => {
            eprintln!("Profiling headless mode for {}s...", duration_secs);
            let mut atari = HeadlessAtari::new(rom);
            for _ in 0..10 { atari.run_frame(); }

            while Instant::now() < deadline {
                atari.set_action(actions[frames as usize % actions.len()]);
                cycles += atari.run_frame();
                frames += 1;
                coz::progress!("frame");
            }
        }
        "rendering" => {
            eprintln!("Profiling rendering mode for {}s...", duration_secs);
            let mut atari = Atari::new(rom);
            for _ in 0..10 { atari.run_frame(); }

            while Instant::now() < deadline {
                atari.set_action(actions[frames as usize % actions.len()]);
                cycles += atari.run_frame();
                frames += 1;
                coz::progress!("frame");
            }
        }
        _ => {
            eprintln!("Unknown mode: {} (use 'headless' or 'rendering')", mode);
            std::process::exit(1);
        }
    }

    let elapsed = duration_secs as f64;
    eprintln!("{} frames in {:.1}s = {:.0} fps, {:.2e} cycles/sec",
        frames, elapsed, frames as f64 / elapsed, cycles as f64 / elapsed);
}

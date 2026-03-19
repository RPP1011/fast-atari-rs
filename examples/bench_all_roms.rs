/// Benchmark every ROM in a directory. Single-threaded headless.
///
/// Usage:
///   cargo run --release --example bench_all_roms -- <ROM_DIR> [SECONDS_PER_ROM]

use std::env;
use std::fs;
use std::time::{Duration, Instant};

use fast_atari_rs::atari::{Action, HeadlessAtari};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ROM_DIR> [SECONDS_PER_ROM]", args[0]);
        std::process::exit(1);
    }

    let rom_dir = &args[1];
    let duration_secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);

    let mut rom_files: Vec<_> = fs::read_dir(rom_dir)
        .expect("failed to read ROM directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "bin"))
        .collect();
    rom_files.sort_by_key(|e| e.file_name());

    eprintln!("Benchmarking {} ROMs, {}s each", rom_files.len(), duration_secs);

    // Cycle through all 18 actions, matching ALE benchmark
    let actions: Vec<Action> = (0..18).map(Action::from_index).collect();
    let mut results = Vec::new();

    for entry in &rom_files {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let rom = fs::read(&path).expect("failed to read ROM");
        let rom_size = rom.len();

        eprint!("  {:<30} ({:>5} bytes) ... ", name, rom_size);

        // Use catch_unwind to handle ROMs that hit illegal opcodes
        let rom_clone = rom.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut atari = HeadlessAtari::new(rom_clone);
            // Warm up
            for _ in 0..30 {
                atari.run_frame();
            }

            let start = Instant::now();
            let deadline = start + Duration::from_secs(duration_secs);
            let mut frames: u64 = 0;

            while Instant::now() < deadline {
                atari.set_action(actions[frames as usize % actions.len()]);
                atari.run_frame();
                frames += 1;
            }

            let elapsed = start.elapsed().as_secs_f64();
            frames as f64 / elapsed
        }));

        match result {
            Ok(fps) => {
                eprintln!("{:>8.0} fps", fps);
                results.push(format!(
                    "  {{\"rom\": \"{}\", \"rom_size\": {}, \"status\": \"ok\", \"fps\": {:.1}}}",
                    name, rom_size, fps
                ));
            }
            Err(_) => {
                eprintln!("CRASH");
                results.push(format!(
                    "  {{\"rom\": \"{}\", \"rom_size\": {}, \"status\": \"crash\", \"fps\": 0}}",
                    name, rom_size
                ));
            }
        }
    }

    let json = format!("[\n{}\n]", results.join(",\n"));
    fs::write("bench_all_fast_atari.json", &json).expect("failed to write JSON");
    eprintln!("\nResults written to bench_all_fast_atari.json");
}

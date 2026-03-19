/// Headless benchmark: measures emulator throughput across varying core counts.
///
/// Usage:
///   cargo run --release --example benchmark -- <ROM> [MAX_THREADS] [SECONDS_PER_RUN]
///
/// Benchmarks both rendering and headless modes, writing results to JSON for plotting.

use std::env;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use fast_atari_rs::atari::{Action, Atari, HeadlessAtari};

#[derive(Clone, Copy)]
enum Mode {
    Rendering,
    Headless,
}

fn run_bench(rom: &[u8], num_threads: usize, duration_secs: u64, mode: Mode) -> BenchResult {
    let stop = Arc::new(AtomicBool::new(false));
    let total_frames = Arc::new(AtomicU64::new(0));
    let total_cycles = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let rom = rom.to_vec();
            let stop = Arc::clone(&stop);
            let total_frames = Arc::clone(&total_frames);
            let total_cycles = Arc::clone(&total_cycles);

            thread::spawn(move || {
                let actions = [Action::Noop, Action::Fire, Action::Left, Action::Right];
                let mut action_idx: usize = 0;
                let mut frames: u64 = 0;
                let mut cycles: u64 = 0;

                match mode {
                    Mode::Rendering => {
                        let mut atari = Atari::new(rom);
                        for _ in 0..10 { atari.run_frame(); }
                        while !stop.load(Ordering::Relaxed) {
                            atari.set_action(actions[action_idx % actions.len()]);
                            cycles += atari.run_frame();
                            frames += 1;
                            action_idx += 1;
                        }
                    }
                    Mode::Headless => {
                        let mut atari = HeadlessAtari::new(rom);
                        for _ in 0..10 { atari.run_frame(); }
                        while !stop.load(Ordering::Relaxed) {
                            atari.set_action(actions[action_idx % actions.len()]);
                            cycles += atari.run_frame();
                            frames += 1;
                            action_idx += 1;
                        }
                    }
                }

                total_frames.fetch_add(frames, Ordering::Relaxed);
                total_cycles.fetch_add(cycles, Ordering::Relaxed);
                frames
            })
        })
        .collect();

    thread::sleep(Duration::from_secs(duration_secs));
    stop.store(true, Ordering::Relaxed);

    let mut per_thread_frames = Vec::new();
    for h in handles {
        per_thread_frames.push(h.join().unwrap());
    }

    let elapsed = start.elapsed().as_secs_f64();
    let frames = total_frames.load(Ordering::Relaxed);
    let cycles = total_cycles.load(Ordering::Relaxed);

    BenchResult {
        threads: num_threads,
        elapsed,
        total_frames: frames,
        total_cycles: cycles,
        fps: frames as f64 / elapsed,
        fps_per_thread: frames as f64 / elapsed / num_threads as f64,
        _per_thread_frames: per_thread_frames,
    }
}

struct BenchResult {
    threads: usize,
    elapsed: f64,
    total_frames: u64,
    total_cycles: u64,
    fps: f64,
    fps_per_thread: f64,
    _per_thread_frames: Vec<u64>,
}

fn write_json(results: &[BenchResult], label: &str, path: &str) {
    let mut json = String::from("[\n");
    for (i, r) in results.iter().enumerate() {
        json.push_str(&format!(
            "  {{\"emulator\": \"{}\", \"threads\": {}, \"elapsed\": {:.3}, \"total_frames\": {}, \"total_cycles\": {}, \"fps\": {:.1}, \"fps_per_thread\": {:.1}}}",
            label, r.threads, r.elapsed, r.total_frames, r.total_cycles, r.fps, r.fps_per_thread
        ));
        if i < results.len() - 1 { json.push(','); }
        json.push('\n');
    }
    json.push(']');
    fs::write(path, &json).expect("failed to write results JSON");
}

fn print_table(results: &[BenchResult]) {
    println!("  {:>7} {:>12} {:>12} {:>12}", "Threads", "FPS", "FPS/thread", "Efficiency");
    let single_fps = results[0].fps;
    for r in results {
        let efficiency = r.fps / (single_fps * r.threads as f64) * 100.0;
        println!("  {:>7} {:>12.0} {:>12.0} {:>11.1}%",
            r.threads, r.fps, r.fps_per_thread, efficiency);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ROM> [MAX_THREADS] [SECONDS_PER_RUN]", args[0]);
        std::process::exit(1);
    }

    let rom = fs::read(&args[1]).expect("failed to read ROM file");
    let max_threads: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(num_cpus);
    let duration_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);

    let mut thread_counts = Vec::new();
    let mut t = 1;
    while t <= max_threads {
        thread_counts.push(t);
        t *= 2;
    }
    if *thread_counts.last().unwrap() != max_threads {
        thread_counts.push(max_threads);
    }

    println!("fast-atari-rs scaling benchmark");
    println!("  ROM:             {}", args[1]);
    println!("  Max threads:     {}", max_threads);
    println!("  Duration/run:    {}s", duration_secs);
    println!("  Thread counts:   {:?}", thread_counts);

    // --- Rendering mode ---
    println!("\n  === Rendering TIA ===");
    let mut rendering_results = Vec::new();
    for &n in &thread_counts {
        eprint!("  Running with {:>3} thread(s)... ", n);
        let r = run_bench(&rom, n, duration_secs, Mode::Rendering);
        eprintln!("{:>8.0} fps  ({:.0} fps/thread)", r.fps, r.fps_per_thread);
        rendering_results.push(r);
    }

    // --- Headless mode ---
    println!("\n  === Headless TIA ===");
    let mut headless_results = Vec::new();
    for &n in &thread_counts {
        eprint!("  Running with {:>3} thread(s)... ", n);
        let r = run_bench(&rom, n, duration_secs, Mode::Headless);
        eprintln!("{:>8.0} fps  ({:.0} fps/thread)", r.fps, r.fps_per_thread);
        headless_results.push(r);
    }

    // Write results
    write_json(&rendering_results, "fast-atari-rs", "benchmark_fast_atari_rs.json");
    write_json(&headless_results, "fast-atari-rs (headless)", "benchmark_fast_atari_rs_headless.json");

    println!("\n  --- Rendering ---");
    print_table(&rendering_results);
    println!("\n  --- Headless ---");
    print_table(&headless_results);

    // Speedup
    println!("\n  --- Headless speedup ---");
    println!("  {:>7} {:>12} {:>12} {:>10}", "Threads", "Rendering", "Headless", "Speedup");
    for (r, h) in rendering_results.iter().zip(headless_results.iter()) {
        println!("  {:>7} {:>12.0} {:>12.0} {:>9.1}x",
            r.threads, r.fps, h.fps, h.fps / r.fps);
    }
}

fn num_cpus() -> usize {
    thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

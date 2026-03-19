#[cfg(feature = "cuda")]
fn main() {
    use fast_atari_rs::cuda_env::BatchAtariGpu;
    use std::time::Instant;

    let rom = std::fs::read("Breakout.bin").expect("Place Breakout.bin in project root");

    for (label, sorted) in [("Phase 2: per-frame", false), ("Phase 4: per-frame sorted", true)] {
        println!("=== {} ===", label);
        for &n in &[1000, 5000, 10000, 50000] {
            let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
            gpu.set_sorted(sorted);
            gpu.reset().unwrap();

            let actions = vec![0u8; n];
            for _ in 0..10 { gpu.step(&actions).unwrap(); }

            let num_frames = 1000;
            let start = Instant::now();
            for _ in 0..num_frames { gpu.step(&actions).unwrap(); }
            let elapsed = start.elapsed();
            let fps = (n as f64 * num_frames as f64) / elapsed.as_secs_f64();
            println!("{:>6} instances × {:>4} frames = {:>10.0} FPS  ({:.2}s)",
                n, num_frames, fps, elapsed.as_secs_f64());
        }
        println!();
    }

    for (label, sorted) in [("Phase 3: multi-frame", false), ("Phase 4: multi-frame sorted", true)] {
        println!("=== {} ===", label);
        for &n in &[1000, 5000, 10000, 50000] {
            let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
            gpu.set_sorted(sorted);
            gpu.reset().unwrap();

            let k = 1000;
            let actions = vec![0u8; k * n];
            gpu.step_multi(&actions, k).unwrap(); // warm up

            let batches = 3;
            let start = Instant::now();
            for _ in 0..batches { gpu.step_multi(&actions, k).unwrap(); }
            let elapsed = start.elapsed();
            let fps = (n as f64 * k as f64 * batches as f64) / elapsed.as_secs_f64();
            println!("{:>6} instances × {:>4}K frames = {:>10.0} FPS  ({:.2}s)",
                n, k * batches / 1000, fps, elapsed.as_secs_f64());
        }
        println!();
    }
}

#[cfg(not(feature = "cuda"))]
fn main() {
    eprintln!("Build with --features cuda");
}

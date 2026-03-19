#[cfg(feature = "cuda")]
fn main() {
    use fast_atari_rs::cuda_env::BatchAtariGpu;
    use std::time::Instant;

    let rom = std::fs::read("Breakout.bin").expect("Place Breakout.bin in project root");

    println!("=== Phase 2: per-frame kernel ===");
    for &n in &[1000, 5000, 10000, 50000] {
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
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

    println!("\n=== Phase 3: multi-frame kernel (K frames per launch) ===");
    for &n in &[1000, 5000, 10000, 50000] {
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.reset().unwrap();

        let k = 1000;
        let actions = vec![0u8; k * n];

        // Warm up
        gpu.step_multi(&actions, k).unwrap();

        let batches = 3;
        let start = Instant::now();
        for _ in 0..batches { gpu.step_multi(&actions, k).unwrap(); }
        let elapsed = start.elapsed();
        let total_frames = n as f64 * k as f64 * batches as f64;
        let fps = total_frames / elapsed.as_secs_f64();
        println!("{:>6} instances × {:>4} frames × {} batches = {:>10.0} FPS  ({:.2}s)",
            n, k, batches, fps, elapsed.as_secs_f64());
    }
}

#[cfg(not(feature = "cuda"))]
fn main() {
    eprintln!("Build with --features cuda");
}

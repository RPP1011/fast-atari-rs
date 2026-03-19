The goal of this is to make a version of ALE that can run on 1M+ (aggregate) SPS on a single 4090.

I am going to start with getting the CPU working in a way that transfers well to the CUDA code that I will have to write later.

All credit to https://github.com/Klaus2m5/6502_65C02_functional_tests for the tests. They are essential to having any idea of what is going on.

~~Note: Cycle count is sometimes wrong. As I am not implmenting a TIA, I will not be fixing that.~~

Turns out it is really easy to fix. There is a difference between pragmatism and laziness.

## Benchmarks

### fast-atari-rs vs ALE (Gymnasium)

![Benchmark scaling](benchmark_scaling.png)

#### All-ROM comparison (101 games, single-threaded headless, vs ALE raw C++)

| Metric | Value |
|---|---|
| **ROMs where fast-atari-rs is faster** | **81/101** |
| ROMs where ALE is faster | 20/101 |
| **Geometric mean ratio (fast-atari / ALE)** | **1.25x** |
| Median ratio | 1.42x |
| Excluding outliers (>100K fps) | 1.17x geomean, 78/98 faster |

#### Headless throughput (Breakout, single-threaded)

| Emulator | FPS |
|---|---|
| **fast-atari-rs (headless, cpu-optimized)** | **40,120** |
| ALE-raw (headless, no obs) | 16,240 |
| ALE-raw (screen obs) | 13,164 |
| ALE (gymnasium) | 12,429 |
| fast-atari-rs (rendering) | 2,269 |

#### Aggregate throughput at 32 cores (Breakout)

| Emulator | FPS | Efficiency |
|---|---|---|
| **fast-atari-rs (headless)** | **302,948** | **51.2%** |
| ALE-raw (headless) | 248,310 | 47.8% |
| ALE (gymnasium) | 186,683 | 46.9% |

The `main` branch keeps the architecture intentionally simple (no jump tables, no computed goto) to facilitate a future CUDA port. The `cpu-optimized` branch trades CUDA portability for CPU throughput.

#### CPU-optimized branch optimizations (Breakout)

| Optimization | Single-thread FPS | Cumulative speedup |
|---|---|---|
| Baseline headless | 10,118 | 1.0x |
| + WSYNC fast-forward | 12,283 | 1.21x |
| + Flat 8KB memory bus | 16,167 | 1.60x |
| + Static opcode tables | 17,127 | 1.69x |
| + Inline bankswitch elision | 18,479 | 1.83x |
| + Batched TIA/PIA ticking | 22,487 | 2.22x |
| + Function pointer dispatch table | 40,120 | **3.97x** |

### Running benchmarks

```bash
# fast-atari-rs scaling benchmark (rendering + headless)
cargo run --release --example benchmark -- <ROM> [MAX_THREADS] [SECONDS]

# All-ROM sweep (single-threaded headless)
cargo run --release --example bench_all_roms -- roms/ [SECONDS_PER_ROM]
python bench_all_ale.py [SECONDS_PER_ROM]

# ALE comparison (requires gymnasium + ale-py in .venv)
.venv/bin/python bench_ale.py [MAX_WORKERS] [SECONDS]
.venv/bin/python bench_ale_raw.py [MAX_WORKERS] [SECONDS]

# Generate comparison plot
.venv/bin/python plot_benchmark.py
```
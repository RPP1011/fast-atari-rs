The goal of this is to make a version of ALE that can run on 1M+ (aggregate) SPS on a single 4090.

I am going to start with getting the CPU working in a way that transfers well to the CUDA code that I will have to write later.

All credit to https://github.com/Klaus2m5/6502_65C02_functional_tests for the tests. They are essential to having any idea of what is going on.

~~Note: Cycle count is sometimes wrong. As I am not implmenting a TIA, I will not be fixing that.~~

Turns out it is really easy to fix. There is a difference between pragmatism and laziness.

## Benchmarks

### fast-atari-rs vs ALE (Gymnasium)

![Benchmark scaling](benchmark_scaling.png)

#### Headless throughput (Breakout, single-threaded)

| Emulator | FPS |
|---|---|
| **fast-atari-rs (headless, cpu-optimized)** | **18,479** |
| ALE-raw (headless, no obs) | 16,240 |
| ALE-raw (screen obs) | 13,164 |
| ALE (gymnasium) | 12,429 |
| fast-atari-rs (rendering) | 2,269 |

#### Aggregate throughput at 32 cores

| Emulator | FPS | Efficiency |
|---|---|---|
| **fast-atari-rs (headless)** | **302,948** | **51.2%** |
| ALE-raw (headless) | 248,310 | 47.8% |
| ALE (gymnasium) | 186,683 | 46.9% |

fast-atari-rs headless is **1.14x faster** than ALE's raw C++ core single-threaded, and **1.22x faster** at 32 cores. The architecture is intentionally simple (no jump tables, no computed goto) to facilitate a future CUDA port.

#### CPU-optimized branch optimizations

| Optimization | Single-thread FPS | Cumulative speedup |
|---|---|---|
| Baseline headless | 10,118 | 1.0x |
| + WSYNC fast-forward | 12,283 | 1.21x |
| + Flat 8KB memory bus | 16,167 | 1.60x |
| + Static opcode tables | 17,127 | 1.69x |
| + Inline bankswitch elision | 18,479 | 1.83x |

### Profiling (headless mode, cpu-optimized)

| Function | % time |
|---|---|
| `run_one_cycle` (frame loop + TIA/PIA tick) | 33.9% |
| `Cpu::step` (instruction dispatch) | 29.2% |
| `HeadlessBus::read` (flat array lookup) | 23.9% |
| `Cpu::resolve_addr` | 4.4% |
| Other | 8.6% |

### Running benchmarks

```bash
# fast-atari-rs scaling benchmark (rendering + headless)
cargo run --release --example benchmark -- <ROM> [MAX_THREADS] [SECONDS]

# ALE comparison (requires gymnasium + ale-py in .venv)
.venv/bin/python bench_ale.py [MAX_WORKERS] [SECONDS]
.venv/bin/python bench_ale_raw.py [MAX_WORKERS] [SECONDS]

# Generate comparison plot
.venv/bin/python plot_benchmark.py
```
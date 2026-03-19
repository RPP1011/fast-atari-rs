# fast-atari-rs — CUDA Persistent-Kernel Atari 2600 Emulator

The goal is 0.5–1.5M aggregate FPS on a single RTX 4090 using persistent CUDA kernels with one Atari simulation per thread.

## CUDA Architecture

Each CUDA thread runs a complete Atari 2600 instance (CPU + headless TIA + PIA). State is stored in global memory (Phase 1, AoS). ROM is shared across all instances via a global memory buffer.

### Phase 1: Naive kernel (current) — correctness first

- One thread per instance, all state in global memory AoS
- 149-opcode `switch(opcode)` with inline address resolution
- Bus read/write dispatches to TIA registers, PIA RAM/IO, and bankswitched ROM
- WSYNC fast-forward preserved from CPU implementation
- Bankswitching: Fixed (2K/4K), F8 (8K), F6 (16K), F4 (32K)

### Verification

GPU output is verified against the CPU emulator after every frame:
- 10 instances × 500 frames with varied action sequences — all 128 bytes of PIA RAM compared per frame, CPU registers spot-checked at 5 checkpoints
- 18 instances × 100 frames with all 18 action types simultaneously — full RAM + register comparison

### Phase 1 Performance (RTX 4090, Breakout)

| Instances | Aggregate FPS |
|-----------|--------------|
| 1,000     | 40K          |
| 5,000     | 198K         |
| 10,000    | 265K         |
| 50,000    | 281K         |

### Planned optimizations

| Phase | Description | Expected speedup |
|-------|------------|-----------------|
| 2 | SoA layout + registers + shared memory for PIA RAM | 3–5x |
| 3 | Persistent kernel (state stays in registers across frames) | 1.3–1.8x |
| 4 | Warp-level opcode sorting to reduce divergence | 1.5–2.5x |

## File structure

```
cuda/
  src/
    atari_kernel.cu       — Frame kernel, action dispatch, frame loop
    cpu_6502.cuh          — 149-case switch, inline address resolution
    memory_bus.cuh        — bus_read/bus_write with TIA/PIA/ROM dispatch
    tia_headless.cuh      — tick_n, wsync skip, register read/write
    pia.cuh               — RAM access, timer tick_n, IO ports
    state_layout.cuh      — AtariState struct (AoS)
  tests/
    test_frame.cu         — Standalone compilation test
src/
  cuda_env.rs             — cudarc host-side: BatchAtariGpu struct
  lib.rs                  — pub mod cuda_env (feature-gated)
build.rs                  — nvcc compilation to PTX
tests/
  test_cuda.rs            — GPU vs CPU frame comparison
examples/
  bench_cuda.rs           — Performance benchmark
```

## Building

Requires CUDA Toolkit 12.0+ and an sm_89 GPU (RTX 4090).

```bash
# Run verification tests
cargo test --features cuda --test test_cuda

# Run performance benchmark
cargo run --release --features cuda --example bench_cuda
```

## CPU Emulator Benchmarks

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

#### All-ROM comparison (101 games, single-threaded headless)

| Metric | Value |
|---|---|
| ROMs where fast-atari-rs is faster | 14/101 |
| ROMs where ALE is faster | 87/101 |
| Geometric mean ratio (fast-atari / ALE) | **0.61x** |

The architecture is intentionally simple (no jump tables, no computed goto) to facilitate the CUDA port.

### Running benchmarks

```bash
# fast-atari-rs scaling benchmark (rendering + headless)
cargo run --release --example benchmark -- <ROM> [MAX_THREADS] [SECONDS]

# All-ROM sweep (single-threaded headless)
cargo run --release --example bench_all_roms -- roms/ [SECONDS_PER_ROM]

# ALE comparison (requires gymnasium + ale-py in .venv)
.venv/bin/python bench_ale.py [MAX_WORKERS] [SECONDS]

# Generate comparison plot
.venv/bin/python plot_benchmark.py
```

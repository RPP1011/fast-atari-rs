# fast-atari-rs — CUDA Persistent-Kernel Atari 2600 Emulator

**2.1M aggregate FPS** on a single RTX 4090 with 50K parallel Atari instances.

## CUDA Architecture

Each CUDA thread runs a complete Atari 2600 instance (CPU + headless TIA + PIA). ROM is shared across all instances via a global memory buffer.

### Phase 1: Naive kernel — correctness first

- One thread per instance, all state in global memory AoS
- 149-opcode `switch(opcode)` with inline address resolution
- Bus read/write dispatches to TIA registers, PIA RAM/IO, and bankswitched ROM
- WSYNC fast-forward preserved from CPU implementation
- Bankswitching: Fixed (2K/4K), F8 (8K), F6 (16K), F4 (32K)

### Phase 2: Registers + shared memory (current)

- `ThreadCtx` struct holds all CPU/TIA/PIA state in GPU registers during the frame
- PIA RAM (128 bytes per instance) lives in `__shared__` memory — eliminates global memory traffic for every stack push/pull and RAM read/write
- Global memory touched only at frame boundaries (load at start, store at end)
- 16KB shared memory per block (128 threads × 128 bytes)

### Phase 3: Multi-frame kernel

- `atari_multi_frame_kernel` runs K frames in a single kernel launch
- State stays in registers and shared memory across all K frames — no load/store between frames
- Useful for RL training: batch K frames of pre-computed actions in one call
- Throughput is the same as Phase 2 (per-frame load/store was already <0.1% of frame time)

### Verification

GPU output is verified against the CPU emulator after every frame:
- 10 instances × 500 frames with varied action sequences — all 128 bytes of PIA RAM compared per frame, CPU registers spot-checked at 5 checkpoints
- 18 instances × 100 frames with all 18 action types simultaneously — full RAM + register comparison
- Multi-frame: 10 instances × 500 frames in one kernel launch, 18 instances × 100 frames with per-instance actions

### Performance (RTX 4090, Breakout)

| Instances | Phase 1 | Phase 2 | Phase 3 (multi) |
|-----------|---------|---------|-----------------|
| 1,000     | 40K     | 58K     | 55K             |
| 5,000     | 198K    | 289K    | 273K            |
| 10,000    | 265K    | 575K    | 546K            |
| 50,000    | 281K    | **2,185K** | **2,099K**   |

vs CPU single-threaded baseline (18K FPS): **121x speedup** at 50K instances.

### Planned optimizations

| Phase | Description | Expected speedup |
|-------|------------|-----------------|
| 4 | Warp-level opcode sorting to reduce divergence | 1.5–2.5x |

## File structure

```
cuda/
  src/
    atari_kernel.cu       — Per-frame + multi-frame kernels
    cpu_6502.cuh          — 149-case switch, inline address resolution
    memory_bus.cuh        — bus_read/bus_write with TIA/PIA/ROM dispatch
    tia_headless.cuh      — tick_n, wsync skip, register read/write
    pia.cuh               — RAM in shared memory, timer tick_n, IO ports
    state_layout.cuh      — AtariState (global), ThreadCtx (registers), load/store
  tests/
    test_frame.cu         — Standalone compilation test
src/
  cuda_env.rs             — cudarc host-side: BatchAtariGpu with step() and step_multi()
  lib.rs                  — pub mod cuda_env (feature-gated)
build.rs                  — nvcc compilation to PTX
tests/
  test_cuda.rs            — GPU vs CPU per-frame comparison
  test_cuda_persistent.rs — GPU vs CPU multi-frame comparison
examples/
  bench_cuda.rs           — Performance benchmark (Phase 2 vs Phase 3)
```

## Building

Requires CUDA Toolkit 12.0+ and an sm_89 GPU (RTX 4090).

```bash
# Run verification tests
cargo test --release --features cuda --test test_cuda
cargo test --release --features cuda --test test_cuda_persistent

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

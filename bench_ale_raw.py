"""Raw ALE C++ interface benchmark — no gymnasium wrapper overhead.

Usage:
    python bench_ale_raw.py [MAX_WORKERS] [SECONDS_PER_RUN]

Tests three modes:
  - act() only (no observation fetch — pure emulation speed)
  - act() + getRAM (128-byte observation)
  - act() + getScreenRGB (full pixel observation)
"""

import json
import multiprocessing as mp
import os
import sys
import time


def worker(mode, duration_secs, result_queue):
    import ale_py
    import numpy as np

    ale = ale_py.ALEInterface()
    ale.setInt("random_seed", 0)
    ale.loadROM(ale_py.roms.get_rom_path("breakout"))
    ale.reset_game()

    if mode == "screen":
        screen = np.empty((*ale.getScreenDims(), 3), dtype=np.uint8)
    elif mode == "ram":
        ram = np.empty(ale.getRAMSize(), dtype=np.uint8)

    actions = [ale_py.Action.NOOP, ale_py.Action.FIRE,
               ale_py.Action.RIGHT, ale_py.Action.LEFT]

    frames = 0
    start = time.monotonic()
    deadline = start + duration_secs

    while time.monotonic() < deadline:
        ale.act(actions[frames % len(actions)])
        if mode == "screen":
            ale.getScreenRGB(screen)
        elif mode == "ram":
            ale.getRAM(ram)
        frames += 1
        if ale.game_over():
            ale.reset_game()

    elapsed = time.monotonic() - start
    result_queue.put((frames, elapsed))


def run_bench(mode, num_workers, duration_secs):
    ctx = mp.get_context("spawn")
    q = ctx.Queue()

    procs = []
    for _ in range(num_workers):
        p = ctx.Process(target=worker, args=(mode, duration_secs, q))
        p.start()
        procs.append(p)

    for p in procs:
        p.join()

    total_frames = 0
    max_elapsed = 0.0
    for _ in range(num_workers):
        frames, elapsed = q.get()
        total_frames += frames
        max_elapsed = max(max_elapsed, elapsed)

    fps = total_frames / max_elapsed if max_elapsed > 0 else 0
    fps_per_worker = fps / num_workers if num_workers > 0 else 0

    return {
        "emulator": f"ALE-raw ({mode})",
        "threads": num_workers,
        "elapsed": round(max_elapsed, 3),
        "total_frames": total_frames,
        "fps": round(fps, 1),
        "fps_per_thread": round(fps_per_worker, 1),
    }


def main():
    max_workers = int(sys.argv[1]) if len(sys.argv) > 1 else os.cpu_count() or 1
    duration_secs = int(sys.argv[2]) if len(sys.argv) > 2 else 5

    thread_counts = []
    t = 1
    while t <= max_workers:
        thread_counts.append(t)
        t *= 2
    if thread_counts[-1] != max_workers:
        thread_counts.append(max_workers)

    modes = ["headless", "ram", "screen"]

    print("ALE raw C++ interface scaling benchmark")
    print(f"  Game:            Breakout")
    print(f"  Max workers:     {max_workers}")
    print(f"  Duration/run:    {duration_secs}s")
    print(f"  Worker counts:   {thread_counts}")
    print(f"  Modes:           {modes}")
    print()

    all_results = {}

    for mode in modes:
        print(f"  --- Mode: {mode} ---")
        results = []
        for n in thread_counts:
            print(f"    {n:>3} worker(s)... ", end="", flush=True)
            r = run_bench(mode, n, duration_secs)
            print(f"{r['fps']:>8.0f} fps  ({r['fps_per_thread']:.0f} fps/worker)")
            results.append(r)
        all_results[mode] = results
        print()

    with open("benchmark_ale_raw.json", "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"Results written to benchmark_ale_raw.json")

    # Summary
    for mode in modes:
        results = all_results[mode]
        print(f"\n  {mode}:")
        print(f"  {'Workers':>7} {'FPS':>12} {'FPS/worker':>12} {'Efficiency':>12}")
        single_fps = results[0]["fps"]
        for r in results:
            eff = r["fps"] / (single_fps * r["threads"]) * 100 if single_fps > 0 else 0
            print(f"  {r['threads']:>7} {r['fps']:>12.0f} {r['fps_per_thread']:>12.0f} {eff:>11.1f}%")


if __name__ == "__main__":
    main()

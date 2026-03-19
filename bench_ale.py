"""ALE/Gymnasium scaling benchmark — headless Atari stepping.

Usage:
    python bench_ale.py [MAX_WORKERS] [SECONDS_PER_RUN]

Runs Breakout-v5 with no rendering across 1, 2, 4, ... MAX_WORKERS processes,
writing results to benchmark_ale.json for plotting.
"""

import json
import multiprocessing as mp
import os
import sys
import time


def worker(duration_secs, result_queue):
    """Run a single ALE environment as fast as possible."""
    import ale_py
    import gymnasium as gym
    gym.register_envs(ale_py)

    env = gym.make("ALE/Breakout-v5", frameskip=1, render_mode=None)
    env.reset()

    frames = 0
    start = time.monotonic()
    deadline = start + duration_secs

    while time.monotonic() < deadline:
        action = env.action_space.sample()
        _obs, _reward, terminated, truncated, _info = env.step(action)
        frames += 1
        if terminated or truncated:
            env.reset()

    elapsed = time.monotonic() - start
    env.close()
    result_queue.put((frames, elapsed))


def run_bench(num_workers, duration_secs):
    ctx = mp.get_context("spawn")
    q = ctx.Queue()

    procs = []
    for _ in range(num_workers):
        p = ctx.Process(target=worker, args=(duration_secs, q))
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
        "emulator": "ALE (gymnasium)",
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

    print(f"ALE/Gymnasium scaling benchmark")
    print(f"  Game:            Breakout-v5")
    print(f"  Max workers:     {max_workers}")
    print(f"  Duration/run:    {duration_secs}s")
    print(f"  Worker counts:   {thread_counts}")
    print()

    results = []
    for n in thread_counts:
        print(f"  Running with {n:>3} worker(s)... ", end="", flush=True)
        r = run_bench(n, duration_secs)
        print(f"{r['fps']:>8.0f} fps  ({r['fps_per_thread']:.0f} fps/worker)")
        results.append(r)

    with open("benchmark_ale.json", "w") as f:
        json.dump(results, f, indent=2)
    print(f"\n  Results written to benchmark_ale.json")

    print(f"\n  {'Workers':>7} {'FPS':>12} {'FPS/worker':>12} {'Efficiency':>12}")
    single_fps = results[0]["fps"]
    for r in results:
        eff = r["fps"] / (single_fps * r["threads"]) * 100 if single_fps > 0 else 0
        print(f"  {r['threads']:>7} {r['fps']:>12.0f} {r['fps_per_thread']:>12.0f} {eff:>11.1f}%")


if __name__ == "__main__":
    main()

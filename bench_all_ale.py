"""Benchmark every ALE ROM. Single-threaded, raw ALE interface, act() only.

Usage:
    python bench_all_ale.py [SECONDS_PER_ROM]
"""

import json
import os
import sys
import time

import ale_py


def main():
    duration_secs = int(sys.argv[1]) if len(sys.argv) > 1 else 3

    rom_dir = os.path.dirname(ale_py.roms.get_rom_path("breakout"))
    rom_files = sorted(f for f in os.listdir(rom_dir) if f.endswith(".bin"))

    print(f"Benchmarking {len(rom_files)} ROMs, {duration_secs}s each")

    results = []

    for rom_file in rom_files:
        name = rom_file.removesuffix(".bin")
        rom_path = os.path.join(rom_dir, rom_file)
        rom_size = os.path.getsize(rom_path)

        print(f"  {name:<30} ({rom_size:>5} bytes) ... ", end="", flush=True)

        try:
            ale = ale_py.ALEInterface()
            ale.setInt("random_seed", 0)
            ale.setLoggerMode(ale_py.LoggerMode.Error)
            ale.loadROM(rom_path)
            ale.reset_game()

            # Cycle through all 18 actions to match fast-atari-rs benchmark
            actions = list(range(18))

            frames = 0
            start = time.monotonic()
            deadline = start + duration_secs

            while time.monotonic() < deadline:
                ale.act(actions[frames % len(actions)])
                frames += 1
                if ale.game_over():
                    ale.reset_game()

            elapsed = time.monotonic() - start
            fps = frames / elapsed

            print(f"{fps:>8.0f} fps")
            results.append({
                "rom": name,
                "rom_size": rom_size,
                "status": "ok",
                "fps": round(fps, 1),
            })

        except Exception as e:
            print(f"ERROR: {e}")
            results.append({
                "rom": name,
                "rom_size": rom_size,
                "status": "error",
                "fps": 0,
            })

    with open("bench_all_ale.json", "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults written to bench_all_ale.json")


if __name__ == "__main__":
    main()

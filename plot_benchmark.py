"""Plot scaling benchmark results: stella-rs vs ALE (gymnasium + raw).

Reads benchmark_stella_rs.json, benchmark_ale.json, benchmark_ale_raw.json.
"""

import json
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker


def load_json(path):
    with open(path) as f:
        return json.load(f)


def series(data):
    threads = [r["threads"] for r in data]
    fps = [r["fps"] for r in data]
    fpt = [r["fps_per_thread"] for r in data]
    return threads, fps, fpt


COLORS = {
    "stella-rs":              "#e74c3c",
    "stella-rs (headless)":   "#c0392b",
    "ALE (gymnasium)":        "#95a5a6",
    "ALE-raw (headless)":     "#2c3e50",
    "ALE-raw (ram)":          "#2980b9",
    "ALE-raw (screen)":       "#3498db",
}

MARKERS = {
    "stella-rs":              "o",
    "stella-rs (headless)":   "P",
    "ALE (gymnasium)":        "d",
    "ALE-raw (headless)":     "^",
    "ALE-raw (ram)":          "s",
    "ALE-raw (screen)":       "D",
}


def main():
    out_file = sys.argv[1] if len(sys.argv) > 1 else "benchmark_scaling.png"

    stella = load_json("benchmark_stella_rs.json")
    stella_headless = load_json("benchmark_stella_rs_headless.json")
    ale_gym = load_json("benchmark_ale.json")
    ale_raw = load_json("benchmark_ale_raw.json")

    datasets = [
        ("stella-rs", stella),
        ("stella-rs (headless)", stella_headless),
        ("ALE (gymnasium)", ale_gym),
        ("ALE-raw (headless)", ale_raw["headless"]),
        ("ALE-raw (ram)", ale_raw["ram"]),
        ("ALE-raw (screen)", ale_raw["screen"]),
    ]

    fig, axes = plt.subplots(1, 3, figsize=(18, 6))
    fig.suptitle(
        "Atari Emulator Throughput: stella-rs vs ALE",
        fontsize=14, fontweight="bold",
    )

    for name, data in datasets:
        threads, fps, fpt = series(data)
        color = COLORS[name]
        marker = MARKERS[name]
        lw = 2.5 if name == "stella-rs" else 1.8

        # Plot 1: Aggregate FPS
        axes[0].plot(threads, fps, f"{marker}-", color=color, linewidth=lw,
                     markersize=7, label=name, zorder=3)

        # Plot 2: Per-instance FPS
        axes[1].plot(threads, fpt, f"{marker}-", color=color, linewidth=lw,
                     markersize=7, label=name, zorder=3)

        # Plot 3: Efficiency
        eff = [f / (fps[0] * t) * 100 for f, t in zip(fps, threads)]
        axes[2].plot(threads, eff, f"{marker}-", color=color, linewidth=lw,
                     markersize=7, label=name, zorder=3)

    # Add ideal scaling lines for key series
    for name, data in [("stella-rs (headless)", stella_headless),
                        ("ALE-raw (headless)", ale_raw["headless"])]:
        threads, fps, _ = series(data)
        axes[0].plot(threads, [fps[0] * t for t in threads],
                     "--", color=COLORS[name], alpha=0.25, linewidth=1)

    # Format axes
    ax = axes[0]
    ax.set_xlabel("Parallel instances", fontsize=12)
    ax.set_ylabel("Aggregate frames/sec", fontsize=12)
    ax.set_title("Total Throughput")
    ax.set_xscale("log", base=2)
    ax.set_yscale("log", base=10)
    ax.xaxis.set_major_formatter(ticker.ScalarFormatter())
    ax.legend(fontsize=8, loc="upper left")
    ax.grid(True, alpha=0.3)

    ax = axes[1]
    ax.set_xlabel("Parallel instances", fontsize=12)
    ax.set_ylabel("Frames/sec per instance", fontsize=12)
    ax.set_title("Per-Instance Throughput")
    ax.set_xscale("log", base=2)
    ax.xaxis.set_major_formatter(ticker.ScalarFormatter())
    ax.legend(fontsize=8)
    ax.grid(True, alpha=0.3)

    ax = axes[2]
    ax.axhline(y=100, color="gray", linestyle="--", alpha=0.5, linewidth=1)
    ax.set_xlabel("Parallel instances", fontsize=12)
    ax.set_ylabel("Parallel efficiency (%)", fontsize=12)
    ax.set_title("Scaling Efficiency")
    ax.set_xscale("log", base=2)
    ax.xaxis.set_major_formatter(ticker.ScalarFormatter())
    ax.set_ylim(0, 115)
    ax.legend(fontsize=8)
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig(out_file, dpi=150, bbox_inches="tight")
    print(f"Saved to {out_file}")


if __name__ == "__main__":
    main()

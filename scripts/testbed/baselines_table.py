#!/usr/bin/env python3
"""Tables, criteria and figures for the baseline runs (run_baselines.sh).

Writes OUT/baselines.csv (one row per run), OUT/baselines.md (means over seeds and the pass/fail
criteria) and OUT/figures/*.svg. Every figure is emulation: real processes on one host.

Usage: baselines_table.py OUT
"""

import csv
import glob
import json
import os
import re
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import svgplot  # noqa: E402

NAME = re.compile(r"^(full|static-bucket|vanilla)-(none|replay|flood)(?:-(\d+))?-s(\d+)$")
MODES = ["full", "static-bucket", "vanilla"]
ATTACKS = {"replay": [0, 2, 8, 32], "flood": [0, 40, 100, 200]}


def load(out):
    rows = []
    for path in sorted(glob.glob(f"{out}/*/summary.json")):
        run = os.path.basename(os.path.dirname(path))
        m = NAME.match(run)
        if not m:
            continue
        s = json.load(open(path))
        mode, attack, intensity, seed = m[1], m[2], int(m[3] or 0), int(m[4])
        banned, nbrs = s["byzantine_banned_by"].split("/")
        cpu = s.get("cpu_seconds_per_process", {})
        aw = s["attack_window"]
        rows.append({
            "run": run, "mode": mode, "attack": attack, "intensity": intensity, "seed": seed,
            "neighbours": int(nbrs), "banned_by": int(banned),
            "mdr": aw["delivery"].get("mean"), "mdr_min": aw["delivery"].get("min"),
            "goodput_per_s": aw["goodput_per_s"], "attack_messages": aw["messages"],
            "strikes_counted": s["log_counters"].get("MALICIOUS REPLAY", 0),
            "honest_false_evictions": s["honest_false_evictions"],
            "honest_rate_drops": s["rate_limit_drops"].get("from_honest", 0),
            "origin_drops_total": sum(s.get("origin_budget_drops", {}).values()),
            "frames_per_message": s["frames_received_per_message"],
            "cpu_s_neighbour_mean": cpu.get("byzantine_neighbours_mean"),
            "cpu_s_byzantine": cpu.get("byzantine"),
            "host_cpu_mean": s["host"]["cpu_busy_pct"].get("mean"),
            "host_cpu_max": s["host"]["cpu_busy_pct"].get("max"),
            "mem_mib": s["host"]["mem_used_mib_max"],
            "faithful": s["fidelity"]["pass"],
        })
    return rows


def mean(values):
    v = [x for x in values if x is not None]
    return round(sum(v) / len(v), 3) if v else None


def criteria(rows):
    """Pass/fail criteria, each fixed before the data it judges (docs/benchmarks.md, baselines)."""
    replay = [r for r in rows if r["attack"] == "replay"]
    full_replay = [r for r in replay if r["mode"] == "full"]
    baseline_runs = [r for r in rows if r["mode"] != "full"]
    full = [r for r in rows if r["mode"] == "full"]
    return [
        ("full: B banned by every neighbour in every replay run",
         bool(full_replay) and all(r["banned_by"] == r["neighbours"] for r in full_replay)),
        ("static-bucket and vanilla: B banned by no neighbour in any run",
         bool(baseline_runs) and all(r["banned_by"] == 0 for r in baseline_runs)),
        ("full: MDR >= 0.99 at every intensity",
         bool(full) and all((r["mdr"] or 0) >= 0.99 for r in full)),
        ("0 honest false evictions in every run", all(r["honest_false_evictions"] == 0 for r in rows)),
        ("every run faithful (the testbed's fidelity criteria; a failing run is reported, not dropped)",
         all(r["faithful"] for r in rows)),
    ]


def main():
    out = sys.argv[1]
    rows = load(out)
    with open(f"{out}/baselines.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]), lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    by = defaultdict(list)
    for r in rows:
        by[(r["attack"], r["intensity"], r["mode"])].append(r)
    cols = ["mdr", "goodput_per_s", "banned_by", "strikes_counted", "honest_rate_drops",
            "cpu_s_neighbour_mean", "host_cpu_mean", "host_cpu_max", "frames_per_message"]
    lines = ["| attack | intensity | mode | runs | " + " | ".join(cols) + " | faithful runs |",
             "|" + "---|" * (len(cols) + 5)]
    order = {m: i for i, m in enumerate(MODES)}
    for (attack, intensity, mode), rs in sorted(by.items(), key=lambda kv: (kv[0][0], kv[0][1], order[kv[0][2]])):
        faithful = sum(1 for r in rs if r["faithful"])
        lines.append(f"| {attack} | {intensity} | {mode} | {len(rs)} | "
                     + " | ".join(str(mean([r[c] for r in rs])) for c in cols)
                     + f" | {faithful}/{len(rs)} |")
    lines += ["", "| criterion | holds |", "|---|---|"]
    lines += [f"| {name} | {'yes' if ok else 'NO'} |" for name, ok in criteria(rows)]
    open(f"{out}/baselines.md", "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))

    os.makedirs(f"{out}/figures", exist_ok=True)

    def series(metric, attack):
        result = {}
        for mode in MODES:
            ys = []
            for intensity in ATTACKS[attack]:
                key = ("none", 0, mode) if intensity == 0 else (attack, intensity, mode)
                ys.append(mean([r[metric] for r in by.get(key, [])]))
            result[mode] = ys
        return result

    for metric, ylabel, ymin, ymax, title in (
            ("mdr", "message delivery ratio (honest messages)", 0.0, 1.0, "Delivery"),
            ("goodput_per_s", "honest deliveries per second", 0.0, None, "Goodput"),
            ("cpu_s_neighbour_mean", "CPU seconds per neighbour of B (60 s run)", 0.0, None, "Cost")):
        for attack, xlab in (("replay", "copies of every frame sent by B (0 = no attack)"),
                             ("flood", "fresh messages per second from B (0 = no attack)")):
            svgplot.line_chart(
                f"{out}/figures/{metric}-{attack}.svg",
                f"{title} against {'replay' if attack == 'replay' else 'a fresh-message flood'}",
                "emulation: real crci-node processes on one host, rgg N = 50, mean of 3 seeds; lines with equal values overlap",
                [str(x) for x in ATTACKS[attack]], series(metric, attack), ylabel, ymin, ymax, xlab)


if __name__ == "__main__":
    main()

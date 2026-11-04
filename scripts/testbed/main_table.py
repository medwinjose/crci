#!/usr/bin/env python3
"""Aggregate the runs of OUT/results.csv (from report.py) per configuration over seeds and print
the main testbed table of docs/benchmarks.md as Markdown. Runs named netem-* are skipped.

Usage: main_table.py OUT
"""

import csv
import re
import sys
from collections import defaultdict


def rng(vals, fmt):
    lo, hi = fmt(min(vals)), fmt(max(vals))
    return lo if lo == hi else f"{lo}–{hi}"


ORDER = ["rgg", "mesh", "sweep", "stress", "load", "src8"]


def order(kv):
    name = kv[0]
    rank = next(i for i, p in enumerate(ORDER) if name.startswith(p))
    return rank, int(kv[1][0]["N"]), int(re.sub(r"\D", "", name.split("-")[0]) or 0)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    groups = defaultdict(list)
    with open(f"{sys.argv[1]}/results.csv", newline="") as f:
        for r in csv.DictReader(f):
            if not r["run"].startswith("netem-"):
                groups[re.sub(r"-s\d+$", "", r["run"])].append(r)
    print("| Run | N | Seeds | Mean degree, diameter | Faithful | Delivery after eviction (min) | "
          "Latency median / p95 after eviction | B banned by all neighbours | Eviction latency (median) | "
          "Honest false evictions | Honest frames rate-limited | Hop excess (median) | "
          "Host CPU mean / max | RSS per node |")
    print("|" + "---|" * 14)
    for name, rs in sorted(groups.items(), key=order):
        col = lambda k: [float(r[k]) for r in rs]
        i0, i1 = lambda v: f"{v:.0f}", lambda v: f"{v:.1f}"
        faithful = sum(r["faithful"] == "True" for r in rs)
        banned = sum(r["banned_by"].split("/")[0] == r["banned_by"].split("/")[1] for r in rs)
        print(f"| {name} | {rs[0]['N']} | {len(rs)} | {rng(col('mean_deg'), i1)}, {rng(col('diam'), i0)} "
              f"| {faithful}/{len(rs)} | {min(col('deliv_post')) * 100:.0f}% "
              f"| {rng(col('lat_med_post'), i0)} / {rng(col('lat_p95_post'), i0)} ms "
              f"| {banned}/{len(rs)} | {rng(col('evict_med_ms'), i0)} ms "
              f"| {sum(col('false_evict')):.0f} | {sum(col('rate_drops_honest')):.0f} "
              f"| {rng(col('hop_excess_med'), i1)} ms "
              f"| {rng(col('cpu_mean'), i1)} / {rng(col('cpu_max'), i1)} % "
              f"| {rng(col('rss_mib_node'), i1)} MiB |")


if __name__ == "__main__":
    main()

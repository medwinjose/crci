#!/usr/bin/env python3
"""Tail of the delivery latency after the quarantine, over the runs of one condition.

Reads latency.csv of every run folder given (written by analyze.py), keeps the deliveries of messages
sent after the last neighbour of the replayer banned it (period "post") and prints their 50th, 95th and
99th percentile and the maximum, then the same per run. Percentiles are nearest-rank.

Usage: post_ban_tail.py RUN_DIR [RUN_DIR ...]
"""

import csv
import math
import os
import sys


def pct(values, q):
    values = sorted(values)
    return values[max(math.ceil(q * len(values)) - 1, 0)]


def post_latencies(run_dir):
    with open(os.path.join(run_dir, "latency.csv"), newline="") as f:
        return [float(r["latency_ms"]) for r in csv.DictReader(f) if r["period"] == "post"]


def main():
    runs = sys.argv[1:]
    if not runs:
        sys.exit(__doc__)
    per_run = {run: post_latencies(run) for run in runs}
    pooled = [v for values in per_run.values() for v in values]
    if not pooled:
        sys.exit("no delivery in the post period")
    print(f"runs {len(runs)}  deliveries {len(pooled)}  p50 {pct(pooled, .5):.0f} ms  p95 {pct(pooled, .95):.0f} ms  "
          f"p99 {pct(pooled, .99):.0f} ms  max {max(pooled):.0f} ms")
    for run, values in per_run.items():
        if values:
            print(f"  {os.path.basename(run.rstrip('/'))}: p99 {pct(values, .99):.0f} ms  max {max(values):.0f} ms")


if __name__ == "__main__":
    main()

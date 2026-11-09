#!/usr/bin/env python3
"""Aggregate the netem runs of OUT (results.csv from report.py) per condition.

Writes OUT/netem/<condition>.csv (one row per run of that condition: topology kind, N, seed and the
measured values), OUT/netem/summary.csv (mean and min/max over seeds per condition, kind and N) and
OUT/netem/table.md (the same as a Markdown table). Only runs named netem-* are used; a run that
fails the fidelity criteria is kept in the per-condition CSV with faithful=False and is excluded from
summary.csv, and the number excluded is reported. OUT/netem/conditions.md aggregates each condition
over all its faithful runs (both topologies, every N and seed).

Usage: netem_table.py OUT
"""

import csv
import os
import statistics
import sys
from collections import defaultdict

ORDER = ["l0", "l5", "l10", "l15", "l20", "r5", "r10", "r15", "l10r10"]
METRICS = ["deliv_pre", "deliv_post", "lat_med_pre", "lat_p95_pre", "evict_med_ms", "evict_max_ms",
           "false_evict", "honest_banned", "rate_drops", "cpu_mean"]


def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def banned_frac(banned_by):
    a, b = banned_by.split("/")
    return int(a) / int(b) if int(b) else None


def main():
    out = sys.argv[1]
    with open(f"{out}/results.csv") as f:
        rows = [r for r in csv.DictReader(f) if r["run"].startswith("netem-")]
    os.makedirs(f"{out}/netem", exist_ok=True)
    by_cond = defaultdict(list)
    for r in rows:
        by_cond[r["run"].split("-")[1]].append(r)
    for cond, rs in by_cond.items():
        with open(f"{out}/netem/{cond}.csv", "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(list(rs[0].keys()))
            for r in sorted(rs, key=lambda r: (r["kind"], int(r["N"]), int(r["seed"]))):
                w.writerow(list(r.values()))

    groups = defaultdict(list)
    excluded = 0
    for r in rows:
        if r["faithful"] != "True":
            excluded += 1
            continue
        groups[(r["run"].split("-")[1], r["kind"], int(r["N"]))].append(r)
    header = ["condition", "loss_pct", "reorder_pct", "kind", "N", "runs"] + \
        [f"{m}_mean" for m in METRICS] + ["banned_by_all_runs", "false_evict_max", "deliv_pre_min"]
    table = []
    for (cond, kind, n), rs in sorted(groups.items(), key=lambda kv: (
            ORDER.index(kv[0][0]) if kv[0][0] in ORDER else 99, kv[0][1], kv[0][2])):
        row = [cond, rs[0]["loss_pct"], rs[0]["reorder_pct"], kind, n, len(rs)]
        for m in METRICS:
            vals = [num(r[m]) for r in rs if num(r[m]) is not None]
            row.append(round(statistics.mean(vals), 2) if vals else "")
        full = sum(1 for r in rs if banned_frac(r["banned_by"]) == 1.0)
        row.append(f"{full}/{len(rs)}")
        row.append(max(int(r["false_evict"]) for r in rs))
        dp = [num(r["deliv_pre"]) for r in rs if num(r["deliv_pre"]) is not None]
        row.append(round(min(dp), 3) if dp else "")
        table.append(row)
    with open(f"{out}/netem/summary.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(header)
        w.writerows(table)
    with open(f"{out}/netem/table.md", "w") as f:
        f.write(f"Runs failing the fidelity criteria (excluded): {excluded} of {len(rows)}\n\n")
        f.write("| " + " | ".join(header) + " |\n|" + "---|" * len(header) + "\n")
        for row in table:
            f.write("| " + " | ".join(str(v) for v in row) + " |\n")
    write_conditions(out, rows)
    print(f"{len(rows)} netem runs, {excluded} unfaithful; wrote {out}/netem/")


def write_conditions(out, rows):
    by_cond = defaultdict(list)
    for r in rows:
        if r["faithful"] == "True":
            by_cond[r["run"].split("-")[1]].append(r)
    header = ["Condition", "Runs", "Runs with a false eviction", "Honest false evictions (total, max in one run)",
              "B banned by all its neighbours", "Delivery before the fault, mean",
              "Delivery after B is banned, mean (min of a run)",
              "Eviction latency, median of run medians (max)", "Frames dropped by the rate limit",
              "Relay frames lost (failed delivery)", "Deliveries > 5 s after origination"]

    def total(rs, key):
        vals = [num(r.get(key)) for r in rs]
        return "" if any(v is None for v in vals) else int(sum(vals))

    with open(f"{out}/netem/conditions.md", "w") as f:
        f.write("| " + " | ".join(header) + " |\n|" + "---|" * len(header) + "\n")
        for cond in sorted(by_cond, key=lambda c: ORDER.index(c) if c in ORDER else 99):
            rs = by_cond[cond]
            fe = [int(r["false_evict"]) for r in rs]
            pre = [num(r["deliv_pre"]) for r in rs if num(r["deliv_pre"]) is not None]
            post = [num(r["deliv_post"]) for r in rs if num(r["deliv_post"]) is not None]
            med = [num(r["evict_med_ms"]) for r in rs if num(r["evict_med_ms"]) is not None]
            mx = [num(r["evict_max_ms"]) for r in rs if num(r["evict_max_ms"]) is not None]
            full = sum(1 for r in rs if banned_frac(r["banned_by"]) == 1.0)
            f.write("| " + " | ".join(str(v) for v in [
                cond, len(rs), sum(1 for x in fe if x), f"{sum(fe)}, {max(fe)}", f"{full}/{len(rs)}",
                f"{statistics.mean(pre):.1%}" if pre else "",
                f"{statistics.mean(post):.1%} ({min(post):.0%})" if post else "",
                f"{statistics.median(med) / 1000:.1f} s ({max(mx) / 1000:.1f} s)" if med else "",
                total(rs, "rate_drops"), total(rs, "relay_failed"), total(rs, "late_5s")]) + " |\n")


if __name__ == "__main__":
    main()

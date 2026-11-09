#!/usr/bin/env python3
"""Table, criteria and figures for the Byzantine-ratio runs (run_ratio.sh).

Writes OUT/ratio.csv (one row per run), OUT/ratio.md (mean over seeds per topology, size and
share of attackers, then the pass/fail criteria) and OUT/figures/*.svg.

Attackers replay from the fault on (every message twice), do not originate and do not collude.
Bans are judged between honest observers; delivery between honest nodes; partition is the honest
nodes' graph without the attackers.

The criteria were written before any run of this experiment existed; a criterion that fails is
reported as failed, never reworded.
  1. every run is faithful (the host criteria of the testbed);
  2. no honest node bans another honest node, in any run (including f = 0);
  3. in every run with attackers, every attacker is banned by every honest neighbour (coverage 1);
  4. the slowest ban is at most 30 s after the fault in every run with attackers;
  5. after the quarantine is complete, honest messages reach every honest node of the origin's
     component in every run: the mean delivery inside the component is at least 0.99.
Reported, not judged: the largest honest component, the runs in which the honest graph is
partitioned, the delivery to all honest nodes, and the delivery latency before and after.

Usage: ratio_table.py OUT
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

NAME = re.compile(r"^f(?P<f>\d+)-(?P<kind>rgg|mesh)-(?P<n>\d+)-s(?P<seed>\d+)$")
FRACTIONS = [0, 10, 20, 30, 45]


def mean(v):
    v = [x for x in v if x is not None]
    return round(sum(v) / len(v), 4) if v else None


def get(d, *path):
    for p in path:
        if d is None:
            return None
        d = d.get(p)
    return d


def load(out):
    rows = []
    for path in sorted(glob.glob(f"{out}/*/summary.json")):
        s = json.load(open(path))
        run = os.path.basename(os.path.dirname(path))
        m = NAME.match(run)
        if not m:
            continue
        f = int(m["f"])
        r = s.get("ratio")
        row = {"run": run, "f": f, "kind": m["kind"], "n": int(m["n"]), "seed": int(m["seed"]),
               "faithful": s["fidelity"]["pass"], "cpu_mean_pct": s["host"]["cpu_busy_pct"].get("mean")}
        if f == 0:
            # No fault: every ban on any node is false; delivery to every honest node is the reference.
            row.update({"attackers": 0, "coverage": None, "false_evictions": s["honest_false_evictions"],
                        "evict_median_ms": None, "evict_max_ms": None, "components": 1,
                        "largest_component": 1.0,
                        "deliv_comp_post": get(s, "delivery_ratio", "pre", "mean")
                        if "pre" in s["delivery_ratio"] else None,
                        "deliv_all_post": get(s, "delivery_ratio", "pre", "mean"),
                        "lat_median_pre_ms": get(s, "latency_ms", "pre", "median"),
                        "lat_p95_pre_ms": get(s, "latency_ms", "pre", "p95"),
                        "lat_median_post_ms": get(s, "latency_ms", "pre", "median"),
                        "lat_p95_post_ms": get(s, "latency_ms", "pre", "p95")})
        else:
            row.update({"attackers": r["attackers"], "coverage": r["coverage"],
                        "false_evictions": r["honest_false_evictions"],
                        "evict_median_ms": get(r, "eviction_latency_ms", "median"),
                        "evict_max_ms": get(r, "eviction_latency_ms", "max"),
                        "components": r["components"],
                        "largest_component": r["largest_component_fraction"],
                        "deliv_comp_post": get(r, "delivery_in_component", "post", "mean"),
                        "deliv_all_post": get(r, "delivery_all_honest", "post", "mean"),
                        "lat_median_pre_ms": get(r, "latency_ms", "pre", "median"),
                        "lat_p95_pre_ms": get(r, "latency_ms", "pre", "p95"),
                        "lat_median_post_ms": get(r, "latency_ms", "post", "median"),
                        "lat_p95_post_ms": get(r, "latency_ms", "post", "p95")})
        row["partitioned"] = row["components"] > 1
        rows.append(row)
    return rows


def criteria(rows):
    atk = [r for r in rows if r["f"] > 0]
    out = [("every run faithful", all(r["faithful"] for r in rows)),
           ("no honest node bans another honest node, in any run",
            all(r["false_evictions"] == 0 for r in rows)),
           ("every attacker banned by every honest neighbour, in every run with attackers",
            all(r["coverage"] == 1.0 for r in atk)),
           ("slowest ban <= 30 s after the fault, in every run with attackers",
            all((r["evict_max_ms"] if r["evict_max_ms"] is not None else 1e12) <= 30000 for r in atk)),
           ("after the quarantine, mean delivery inside the origin's honest component >= 0.99, in every run",
            all((r["deliv_comp_post"] if r["deliv_comp_post"] is not None else 0) >= 0.99
                for r in rows))]
    return out


def main():
    out = sys.argv[1]
    rows = load(out)
    if not rows:
        sys.exit("no runs")
    with open(f"{out}/ratio.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    by = defaultdict(list)
    for r in rows:
        by[(r["kind"], r["n"], r["f"])].append(r)
    cols = ["attackers", "coverage", "false_evictions", "evict_median_ms", "evict_max_ms",
            "largest_component", "deliv_comp_post", "deliv_all_post", "lat_median_pre_ms",
            "lat_p95_pre_ms", "lat_median_post_ms", "lat_p95_post_ms", "cpu_mean_pct"]
    lines = ["| topology | N | f % | runs | partitioned runs | " + " | ".join(cols) + " |",
             "|" + "---|" * (len(cols) + 5)]
    for (kind, n, f), rs in sorted(by.items()):
        cells = [str(mean([r[c] for r in rs])) for c in cols]
        lines.append(f"| {kind} | {n} | {f} | {len(rs)} | {sum(r['partitioned'] for r in rs)}/{len(rs)} | "
                     + " | ".join(cells) + " |")
    lines += ["", "| criterion | holds |", "|---|---|"]
    lines += [f"| {name} | {'yes' if ok else 'NO'} |" for name, ok in criteria(rows)]
    open(f"{out}/ratio.md", "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))

    os.makedirs(f"{out}/figures", exist_ok=True)
    xl = [f"{f} %" for f in FRACTIONS]
    sizes = sorted({r["n"] for r in rows})
    sub = "emulation: real processes on one host, 3 seeds, mean over seeds"
    for kind in ("rgg", "mesh"):
        lat = {f"N = {n}": [mean([r["lat_p95_post_ms"] for r in by.get((kind, n, f), [])]) for f in FRACTIONS]
               for n in sizes}
        svgplot.line_chart(f"{out}/figures/latency-vs-attackers-{kind}.svg",
                           f"Honest delivery latency, p95 ({kind})", sub, xl, lat, "milliseconds",
                           xlabel="share of nodes replaying")
        part = {f"N = {n}": [mean([r["largest_component"] for r in by.get((kind, n, f), [])])
                             for f in FRACTIONS] for n in sizes}
        svgplot.line_chart(f"{out}/figures/component-vs-attackers-{kind}.svg",
                           f"Largest honest component after quarantine ({kind})", sub, xl, part,
                           "share of honest nodes", ymin=0.0, ymax=1.0, xlabel="share of nodes replaying")


if __name__ == "__main__":
    main()

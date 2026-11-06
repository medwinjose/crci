#!/usr/bin/env python3
"""Table and criteria for the link-flapping runs (run_flapping.sh).

Writes OUT/flapping.csv (one row per run) and OUT/flapping.md (mean over seeds per condition,
topology and size; then the pass/fail criteria and whether each holds).

The criteria were written before any run of this experiment existed; a criterion that fails is
reported as failed, never reworded.

Honest runs (no fault: every ban or strike on any node is a false one):
  1. every run is faithful (the host criteria of the testbed, and every planned flap applied);
  2. no node bans any other node, in any run;
  3. no strike (replay, repeated sequence number or invalid signature) on any node, in any run;
  4. every message reaches every node by the end of the drain (no undelivered pair), in any run;
  5. no frame is paced and no relay queue overflows, in any run;
  6. in the 20 % conditions the slowest delivery is at most 60 s after origination (a link is
     down for at most 10 s; recovery is an aborted connection, a re-dial and one sync).
Runs with the replaying relay B:
  7. B is banned by all its neighbours, in every run;
  8. the slowest eviction is at most 30 s after the fault (without flapping the median is about 2 s);
  9. no honest node is banned, in any run.

Usage: flap_table.py OUT
"""

import csv
import glob
import json
import os
import re
import sys
from collections import defaultdict

NAME = re.compile(r"^(?P<cond>[a-z0-9-]+?)-(?P<kind>rgg|mesh)-(?P<n>\d+)-s(?P<seed>\d+)$")


def mean(v):
    v = [x for x in v if x is not None]
    return round(sum(v) / len(v), 2) if v else None


def load(out):
    rows = []
    for path in sorted(glob.glob(f"{out}/*/summary.json")):
        s = json.load(open(path))
        run = os.path.basename(os.path.dirname(path))
        m = NAME.match(run)
        if not m:
            continue
        cfg, flap = s["config"], s.get("flap") or {}
        ev = s.get("eviction_latency_ms") or {}
        planned = flap.get("events_planned", 0)
        rows.append({
            "run": run,
            "cond": m["cond"],
            "attack": m["cond"].startswith("atk-"),
            "kind": m["kind"],
            "n": int(m["n"]),
            "seed": int(m["seed"]),
            "flap_fraction": cfg.get("flap_fraction", 0),
            "flap_events": planned,
            "banned_by": s["byzantine_banned_by"],
            "evict_median_ms": ev.get("median"),
            "evict_max_ms": ev.get("max"),
            "honest_false_evictions": s["honest_false_evictions"],
            "bans_total": flap.get("bans_total"),
            "strikes_total": flap.get("strikes_total"),
            "sync_requests": flap.get("sync_requests"),
            "messages": flap.get("messages"),
            "undelivered_pairs": flap.get("undelivered_pairs"),
            "deliv_all_mean": (flap.get("delivery_all_nodes") or {}).get("mean"),
            "lat_median_ms": (flap.get("latency_all_ms") or {}).get("median"),
            "lat_p95_ms": (flap.get("latency_all_ms") or {}).get("p95"),
            "lat_max_ms": (flap.get("latency_all_ms") or {}).get("max"),
            "relay_failed": s["relay_drops"]["failed"],
            "relay_queue_full": s["relay_drops"]["queue_full"],
            "frames_paced": s["frames_paced"]["total"],
            "rate_limited_honest": s["rate_limit_drops"].get("from_honest", 0),
            "cpu_mean_pct": s["host"]["cpu_busy_pct"].get("mean"),
            "faithful": s["fidelity"]["pass"],
        })
    return rows


def criteria(rows):
    honest = [r for r in rows if not r["attack"]]
    attack = [r for r in rows if r["attack"]]
    low = [r for r in honest if r["flap_fraction"] in (0, 0.2)]
    out = [("every run faithful", all(r["faithful"] for r in rows))]
    out.append(("no node bans another in any honest run",
                all(r["bans_total"] == 0 for r in honest)))
    out.append(("no strike on any node in any honest run",
                all(r["strikes_total"] == 0 for r in honest)))
    out.append(("every message reaches every node by the end of the drain, in every honest run",
                all(r["undelivered_pairs"] == 0 for r in honest)))
    out.append(("no frame paced and no relay queue overflow, in every honest run",
                all(r["frames_paced"] == 0 and r["relay_queue_full"] == 0 for r in honest)))
    out.append(("slowest delivery <= 60 s in the reference and 20 % runs",
                all((r["lat_max_ms"] or 1e12) <= 60000 for r in low)))
    if attack:
        out.append(("B banned by all its neighbours in every attack run",
                    all(r["banned_by"].split("/")[0] == r["banned_by"].split("/")[1] for r in attack)))
        out.append(("slowest eviction <= 30 s after the fault in every attack run",
                    all((r["evict_max_ms"] or 1e12) <= 30000 for r in attack)))
        out.append(("no honest node banned in any attack run",
                    all(r["honest_false_evictions"] == 0 for r in attack)))
    return out


def main():
    out = sys.argv[1]
    rows = load(out)
    if not rows:
        sys.exit("no runs")
    with open(f"{out}/flapping.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    by = defaultdict(list)
    for r in rows:
        by[(r["cond"], r["kind"], r["n"])].append(r)
    cols = ["flap_events", "bans_total", "strikes_total", "honest_false_evictions", "banned_by",
            "evict_median_ms", "evict_max_ms", "sync_requests", "undelivered_pairs", "deliv_all_mean",
            "lat_median_ms", "lat_p95_ms", "lat_max_ms", "relay_failed", "relay_queue_full",
            "rate_limited_honest", "frames_paced", "cpu_mean_pct"]
    lines = ["| condition | topology | N | runs | " + " | ".join(cols) + " |",
             "|" + "---|" * (len(cols) + 4)]
    for (cond, kind, n), rs in sorted(by.items(), key=lambda kv: (kv[0][1], kv[0][2], kv[0][0])):
        cells = []
        for c in cols:
            vals = [r[c] for r in rs]
            cells.append(str(vals[0]) if c == "banned_by" and len(set(vals)) == 1
                         else "/".join(vals) if c == "banned_by"
                         else str(mean(vals)))
        lines.append(f"| {cond} | {kind} | {n} | {len(rs)} | " + " | ".join(cells) + " |")
    lines += ["", "| criterion | holds |", "|---|---|"]
    lines += [f"| {name} | {'yes' if ok else 'NO'} |" for name, ok in criteria(rows)]
    open(f"{out}/flapping.md", "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()

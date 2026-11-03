#!/usr/bin/env python3
"""Collect the summary.json of every run in OUT into OUT/results.csv and
print it as a table.

Usage: report.py OUT
"""

import csv
import glob
import json
import os
import sys

COLUMNS = [
    ("run", lambda s: os.path.basename(s["run"])),
    ("kind", lambda s: s["topology"]["kind"]),
    ("seed", lambda s: s["topology"]["seed"]),
    ("N", lambda s: s["topology"]["metrics"]["nodes"]),
    ("delay_ms", lambda s: s["config"]["delay_ms"]),
    ("jitter_ms", lambda s: s["config"].get("jitter_ms", 0)),
    ("loss_pct", lambda s: s["config"].get("loss", 0)),
    ("reorder_pct", lambda s: s["config"].get("reorder", 0)),
    ("mean_deg", lambda s: s["topology"]["metrics"]["mean_degree"]),
    ("max_deg", lambda s: s["topology"]["metrics"]["max_degree"]),
    ("diam", lambda s: s["topology"]["metrics"]["diameter"]),
    ("aspl", lambda s: s["topology"]["metrics"]["avg_shortest_path"]),
    ("clust", lambda s: s["topology"]["metrics"]["clustering"]),
    ("up", lambda s: f"{s['launch']['alive_at_end']}/{s['launch']['nodes']}"),
    ("msgs", lambda s: s["messages_originated_in_window"]),
    ("deliv_pre", lambda s: s["delivery_ratio"].get("pre", {}).get("mean")),
    ("deliv_post", lambda s: s["delivery_ratio"].get("post", {}).get("mean")),
    ("lat_med_pre", lambda s: s["latency_ms"].get("pre", {}).get("median")),
    ("lat_p95_pre", lambda s: s["latency_ms"].get("pre", {}).get("p95")),
    ("lat_med_post", lambda s: s["latency_ms"].get("post", {}).get("median")),
    ("lat_p95_post", lambda s: s["latency_ms"].get("post", {}).get("p95")),
    ("hop_excess_med", lambda s: s["per_hop_excess_ms"].get("median")),
    ("down_pre", lambda s: "/".join(map(str, s["downstream_delivery"].get("pre", ("", ""))))),
    ("down_post", lambda s: "/".join(map(str, s["downstream_delivery"].get("post", ("", ""))))),
    ("via_byz_post", lambda s: s["accepted_via_byzantine"].get("post", 0)),
    ("banned_by", lambda s: s["byzantine_banned_by"]),
    ("evict_med_ms", lambda s: (s["eviction_latency_ms"] or {}).get("median")),
    ("evict_max_ms", lambda s: (s["eviction_latency_ms"] or {}).get("max")),
    ("false_evict", lambda s: s["honest_false_evictions"]),
    ("honest_banned", lambda s: s.get("honest_nodes_banned_by_anyone")),
    ("honest_nodes", lambda s: s.get("honest_nodes")),
    ("rate_drops", lambda s: sum(s["rate_limit_drops"].values())),
    ("rate_drops_honest", lambda s: s["rate_limit_drops"].get("from_honest", 0)),
    ("origin_drops_honest", lambda s: s.get("origin_budget_drops", {}).get("honest_origin", 0)),
    ("relay_failed", lambda s: s.get("relay_drops", {}).get("failed")),
    ("late_5s", lambda s: sum(s.get("late_deliveries_over_5s_by_path", {}).values())),
    ("cpu_mean", lambda s: s["host"]["cpu_busy_pct"].get("mean")),
    ("cpu_max", lambda s: s["host"]["cpu_busy_pct"].get("max")),
    ("mem_mib", lambda s: s["host"]["mem_used_mib_max"]),
    ("swap_io", lambda s: s["host"]["swap_pages_in_out"]),
    ("kdrops", lambda s: s["host"].get("kernel_backlog_drops")),
    ("rss_mib_node", lambda s: round(s["rss_kib_per_node"].get("mean", 0) / 1024, 2)),
    ("rtt_med", lambda s: s["ping_rtt_ms"].get("median")),
    ("rtt_p95", lambda s: s["ping_rtt_ms"].get("p95")),
    ("faithful", lambda s: s["fidelity"]["pass"]),
]


def main():
    out = sys.argv[1]
    rows = []
    for path in sorted(glob.glob(f"{out}/*/summary.json")):
        with open(path) as f:
            s = json.load(f)
        rows.append([fn(s) for _, fn in COLUMNS])
    with open(f"{out}/results.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow([c for c, _ in COLUMNS])
        w.writerows(rows)
    for row in [[c for c, _ in COLUMNS]] + rows:
        print(",".join("" if v is None else str(v) for v in row))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Table and figure for the invalid-signature flood runs (run_flood.sh).

Writes OUT/flood.csv (one row per run), OUT/flood.md (mean over seeds per configuration, the
success criteria and whether each holds) and OUT/figures/flood-cost.svg.

Usage: flood_table.py OUT [BEFORE_DIR]
"""

import csv
import glob
import json
import os
import re
import sys
from collections import defaultdict

VERIFY_US = 37.0  # Ed25519 verify, Task 11 (i5-12450HX)


def readmission(out):
    """Criterion 10 from the readmission run: neighbours that readmitted B, and B's messages
    accepted via B after the first readmission (None when there is no such run)."""
    path = f"{out}/readmit-s1"
    if not os.path.exists(f"{path}/summary.json"):
        return None
    s = json.load(open(f"{path}/summary.json"))
    byz = s["roles"]["byzantine"]
    readmit_ms, accepted = [], []
    accept = re.compile(rf"accepted '[^']+' \(origin {byz}, via {byz}#\S*\) at_ms=(\d+)")
    for log in glob.glob(f"{path}/logs/*.log"):
        for line in open(log, errors="replace"):
            m = re.search(rf"readmitted {byz}#\S+ at_ms=(\d+)", line)
            if m:
                readmit_ms.append(int(m[1]))
            elif m := accept.search(line):
                accepted.append(int(m[1]))
    first = min(readmit_ms) if readmit_ms else None
    return {"neighbours": s["roles"]["byzantine_degree"], "readmitted": len(readmit_ms),
            "accepted_after_readmission": sum(1 for t in accepted if first and t >= first)}


def pacing_only_at_neighbours(run_dir, s):
    """True when every node that paced a connection is a neighbour of B (None: no edge list)."""
    nodes = s.get("frames_paced", {}).get("nodes") or []
    edges = f"{run_dir}/topo_edges.json"
    if not os.path.exists(edges):
        return None
    byz = int(s["roles"]["byzantine"][1:])
    nbrs = {f"n{(a if b == byz else b):03d}" for a, b in json.load(open(edges)) if byz in (a, b)}
    return all(n in nbrs for n in nodes)


def load(out):
    rows = []
    for path in sorted(glob.glob(f"{out}/*/summary.json")):
        if "readmit" in path:
            continue
        s = json.load(open(path))
        run = os.path.basename(os.path.dirname(path))
        cfg = s["config"]
        deg = s["roles"]["byzantine_degree"]
        c = s["log_counters"]
        logs = sum(os.path.getsize(p) for p in glob.glob(f"{os.path.dirname(path)}/logs/*.log"))
        copies = cfg["replay_copies"] if cfg.get("corrupt_signatures") else 0
        verifies = c.get("INVALID SIGNATURE", 0)
        rows.append({
            "run": run,
            "config": run.rsplit("-s", 1)[0],
            "seed": s["topology"]["seed"],
            "forged_frames_per_s_per_link": 0 if copies == 0 else int(cfg["byz_rate"] * copies),
            "neighbours": deg,
            "banned_by": s["byzantine_banned_by"],
            "evict_median_ms": (s["eviction_latency_ms"] or {}).get("median"),
            "evict_max_ms": (s["eviction_latency_ms"] or {}).get("max"),
            "kind": ("forged" if copies else "valid" if run.startswith("valid-")
                     else "fresh" if run.startswith("fresh-") else "reference"),
            "load_matched": run.startswith("valid-"),
            "invalid_verifications": verifies,
            "verifications_per_neighbour": round(verifies / deg, 2),
            "verify_cpu_ms_total": round(verifies * VERIFY_US / 1000, 2),
            "pacing_only_at_byz_neighbours": pacing_only_at_neighbours(os.path.dirname(path), s),
            "frames_paced": s.get("frames_paced", {}).get("total"),
            "nodes_pacing": s.get("frames_paced", {}).get("nodes"),
            "frames_dropped_banned": s.get("banned_frames_dropped", c.get("is banned", 0)),
            "honest_false_evictions": s["honest_false_evictions"],
            "deliv_pre": s["delivery_ratio"].get("pre", {}).get("mean"),
            "deliv_post": s["delivery_ratio"].get("post", {}).get("mean"),
            "down_post": "/".join(map(str, s["downstream_delivery"].get("post", ("", "")))),
            "cpu_mean_pct": s["host"]["cpu_busy_pct"].get("mean"),
            "cpu_max_pct": s["host"]["cpu_busy_pct"].get("max"),
            "mem_used_mib_max": s["host"]["mem_used_mib_max"],
            "rss_mib_per_node": round(s["rss_kib_per_node"].get("mean", 0) / 1024, 2),
            "logs_mib": round(logs / 2**20, 2),
            "cpu_s_byzantine": s.get("cpu_seconds_per_process", {}).get("byzantine"),
            "cpu_s_neighbour_mean": s.get("cpu_seconds_per_process", {}).get("byzantine_neighbours_mean"),
            "cpu_s_neighbour_max": s.get("cpu_seconds_per_process", {}).get("byzantine_neighbours_max"),
            "cpu_s_other_mean": s.get("cpu_seconds_per_process", {}).get("others_mean"),
            "faithful": s["fidelity"]["pass"],
        })
    return rows


def mean(v):
    v = [x for x in v if x is not None]
    return round(sum(v) / len(v), 2) if v else None


def criteria(rows):
    """Fixed before the first run (EVIDENCE.md, Task 15)."""
    flood = [r for r in rows if r["forged_frames_per_s_per_link"]]
    ref = {r["seed"]: r for r in rows if r["kind"] == "reference"}
    matched = {r["seed"]: r for r in rows if r["load_matched"]}
    out = []
    out.append(("every run faithful", all(r["faithful"] for r in rows)))
    out.append(("B banned by all its neighbours in every flood run",
                all(r["banned_by"].split("/")[0] == r["banned_by"].split("/")[1] for r in flood)))
    out.append(("eviction median <= 1 s and max <= 5 s in every flood run",
                all((r["evict_median_ms"] or 1e9) <= 1000 and (r["evict_max_ms"] or 1e9) <= 5000
                    for r in flood)))
    out.append(("<= 3 signature verifications per neighbour in every flood run",
                all(r["verifications_per_neighbour"] <= 3 for r in flood)))
    out.append(("0 honest false evictions in every run",
                all(r["honest_false_evictions"] == 0 for r in rows)))
    # The reference run has no ban, hence no "after the ban" period: it is judged on the
    # whole run. (The first version of this check read the missing value as 0 and failed.)
    out.append(("mean honest delivery after the ban (whole run for the references) >= 0.99",
                all((r["deliv_post"] if r["forged_frames_per_s_per_link"] else r["deliv_pre"]) is not None
                    and (r["deliv_post"] if r["forged_frames_per_s_per_link"] else r["deliv_pre"]) >= 0.99
                    for r in rows if r["kind"] != "fresh")))  # the fresh flood is measured, not judged
    out.append(("host memory within 10 % of the reference run of the same seed",
                all(r["mem_used_mib_max"] <= 1.10 * ref[r["seed"]]["mem_used_mib_max"]
                    for r in flood if r["seed"] in ref)))
    # First version compared with the quiet reference (B silent), which carries none of B's 40
    # messages/s; it failed for that reason (EVIDENCE.md, Task 15). The load-matched reference
    # sends the same messages with valid signatures.
    out.append(("total log size within 2x of the load-matched reference of the same seed",
                all(r["logs_mib"] <= 2 * matched[r["seed"]]["logs_mib"]
                    for r in flood if r["seed"] in matched)))
    rd = readmission(OUT_DIR)
    if rd is not None:
        out.append((f"readmission run: all {rd['neighbours']} neighbours readmit B and accept its "
                    f"messages again (readmitted {rd['readmitted']}, accepted after {rd['accepted_after_readmission']})",
                    rd["readmitted"] == rd["neighbours"] and rd["accepted_after_readmission"] >= 1))
    # Criterion 9 (fixed before the CPU data existed): the work a banned peer can still make its
    # neighbours do is small, judged on the heaviest flood (64 copies, 2 560 frames/s per link).
    heavy = [r for r in flood if r["forged_frames_per_s_per_link"] >= 2560
             and r["cpu_s_neighbour_mean"] is not None]
    out.append(("neighbour CPU seconds at 2 560 forged frames/s per link <= 1.5x the load-matched reference",
                bool(heavy) and all(
                    r["cpu_s_neighbour_mean"] <= 1.5 * matched[r["seed"]]["cpu_s_neighbour_mean"]
                    for r in heavy if r["seed"] in matched)))
    # Criterion (e), fixed before the data of the binary with the paced-frames counter: pacing
    # never acts on an honest link.
    out.append(("no frame is paced in the reference runs", all(
        not r["frames_paced"] for r in rows if r["kind"] in ("reference", "valid"))))
    out.append(("in every flood run only B's neighbours ever pace a connection", all(
        r["pacing_only_at_byz_neighbours"] in (True, None) for r in rows)))
    # Criterion (b), fixed before the paced binary's data: a flood of valid fresh messages
    # (never struck, so never shunned) is bounded by the per-connection frame pacing.
    fresh = [r for r in rows if r["kind"] == "fresh" and r["cpu_s_neighbour_mean"] is not None]
    if fresh:
        out.append(("fresh flood (1 000 messages/s): neighbour CPU seconds <= 2.5x the load-matched reference",
                    all(r["cpu_s_neighbour_mean"] <= 2.5 * matched[r["seed"]]["cpu_s_neighbour_mean"]
                        for r in fresh if r["seed"] in matched)))
        out.append(("fresh flood: total log size <= 3x the load-matched reference",
                    all(r["logs_mib"] <= 3 * matched[r["seed"]]["logs_mib"]
                        for r in fresh if r["seed"] in matched)))
        out.append(("fresh flood: honest delivery >= 0.99",
                    all((r["deliv_pre"] or 0) >= 0.99 for r in fresh)))
    return out


def figure(path, groups):
    """Bars: mean signature verifications per neighbour per configuration."""
    w, h, left, bottom = 520, 260, 60, 50
    top = max([g[1] for g in groups] + [3]) * 1.2
    bw = (w - left - 20) / len(groups)
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" font-family="sans-serif" '
             f'font-size="11"><rect width="{w}" height="{h}" fill="white"/>',
             f'<text x="{left}" y="16" font-size="12">Signature verifications spent on forged frames, per neighbour</text>']
    for i, (label, val) in enumerate(groups):
        x = left + i * bw + bw * 0.2
        bar = (h - bottom - 24) * val / top
        parts.append(f'<rect x="{x:.0f}" y="{h - bottom - bar:.0f}" width="{bw * 0.6:.0f}" height="{bar:.0f}" fill="#4477aa"/>')
        parts.append(f'<text x="{x:.0f}" y="{h - bottom - bar - 4:.0f}">{val}</text>')
        parts.append(f'<text x="{x:.0f}" y="{h - bottom + 14}">{label}</text>')
    parts.append(f'<line x1="{left}" y1="{h - bottom}" x2="{w - 10}" y2="{h - bottom}" stroke="black"/>')
    parts.append(f'<text x="{left}" y="{h - 6}">frames per second on each of B\'s links (mean of seeds)</text></svg>')
    open(path, "w").write("\n".join(parts) + "\n")


OUT_DIR = ""


def cpu_figure(path, before, after):
    """Mean CPU seconds of B's neighbours per forged-frame rate: before and after the shun."""
    def series(rows):
        by = defaultdict(list)
        for r in rows:
            if r["forged_frames_per_s_per_link"] and r["cpu_s_neighbour_mean"] is not None:
                by[r["forged_frames_per_s_per_link"]].append(r["cpu_s_neighbour_mean"])
        return {k: mean(v) for k, v in sorted(by.items())}
    b, a = series(before), series(after)
    matched = mean([r["cpu_s_neighbour_mean"] for r in after if r["load_matched"]])
    w, h, left, bottom = 560, 300, 60, 56
    top = max(list(b.values()) + list(a.values()) + [1]) * 1.15
    rates = sorted(a)
    group = (w - left - 20) / len(rates)
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" font-family="sans-serif" font-size="11">'
             f'<rect width="{w}" height="{h}" fill="white"/>',
             f'<text x="{left}" y="16" font-size="12">CPU seconds spent by each neighbour of the forging node (mean of 3 seeds)</text>']
    for i, rate in enumerate(rates):
        for j, (val, colour) in enumerate(((b.get(rate), "#cc6677"), (a.get(rate), "#4477aa"))):
            if val is None:
                continue
            x = left + i * group + group * (0.12 + 0.4 * j)
            bar = (h - bottom - 28) * val / top
            parts.append(f'<rect x="{x:.0f}" y="{h - bottom - bar:.0f}" width="{group * 0.36:.0f}" height="{bar:.0f}" fill="{colour}"/>')
            parts.append(f'<text x="{x:.0f}" y="{h - bottom - bar - 4:.0f}">{val}</text>')
        parts.append(f'<text x="{left + i * group + group * 0.2:.0f}" y="{h - bottom + 14}">{int(rate)} frames/s per link</text>')
    ym = h - bottom - (h - bottom - 28) * matched / top
    parts.append(f'<line x1="{left}" y1="{ym:.0f}" x2="{w - 10}" y2="{ym:.0f}" stroke="#555" stroke-dasharray="4 3"/>')
    parts.append(f'<text x="{w - 250}" y="{ym - 4:.0f}">same load, valid signatures: {matched}</text>')
    parts.append(f'<line x1="{left}" y1="{h - bottom}" x2="{w - 10}" y2="{h - bottom}" stroke="black"/>')
    parts.append(f'<rect x="{left}" y="{h - 30}" width="10" height="10" fill="#cc6677"/><text x="{left + 14}" y="{h - 21}">before: frames of a banned peer still read and verified</text>')
    parts.append(f'<rect x="{left + 330}" y="{h - 30}" width="10" height="10" fill="#4477aa"/><text x="{left + 344}" y="{h - 21}">after: connection closed</text></svg>')
    open(path, "w").write("\n".join(parts) + "\n")


def main():
    global OUT_DIR
    out = OUT_DIR = sys.argv[1]
    rows = load(out)
    with open(f"{out}/flood.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    by = defaultdict(list)
    for r in rows:
        by[r["config"]].append(r)
    cols = ["forged_frames_per_s_per_link", "evict_median_ms", "evict_max_ms",
            "verifications_per_neighbour", "frames_dropped_banned", "honest_false_evictions",
            "deliv_pre", "deliv_post", "cpu_mean_pct", "cpu_max_pct", "mem_used_mib_max", "rss_mib_per_node", "logs_mib",
            "cpu_s_byzantine", "cpu_s_neighbour_mean", "cpu_s_neighbour_max", "cpu_s_other_mean"]
    lines = ["| config | runs | " + " | ".join(cols) + " |", "|" + "---|" * (len(cols) + 2)]
    groups = []
    for cfg, rs in sorted(by.items(), key=lambda kv: kv[1][0]["forged_frames_per_s_per_link"]):
        lines.append(f"| {cfg} | {len(rs)} | " + " | ".join(str(mean([r[c] for r in rs])) for c in cols) + " |")
        if rs[0]["forged_frames_per_s_per_link"]:
            groups.append((str(rs[0]["forged_frames_per_s_per_link"]), mean([r["verifications_per_neighbour"] for r in rs])))
    lines += ["", "| criterion | holds |", "|---|---|"]
    lines += [f"| {name} | {'yes' if ok else 'NO'} |" for name, ok in criteria(rows)]
    open(f"{out}/flood.md", "w").write("\n".join(lines) + "\n")
    os.makedirs(f"{out}/figures", exist_ok=True)
    figure(f"{out}/figures/flood-cost.svg", groups)
    if len(sys.argv) > 2:  # a directory of runs made before the shun, for the comparison figure
        cpu_figure(f"{out}/figures/flood-cpu.svg", load(sys.argv[2]), rows)
    print("\n".join(lines))


if __name__ == "__main__":
    main()

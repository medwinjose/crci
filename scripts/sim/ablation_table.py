#!/usr/bin/env python3
"""Tables, criteria and figures for the simulation baselines and the strike-threshold ablation.

Reads OUT/results.csv (run_ablation.py); writes OUT/ablation.md, OUT/k_tradeoff.csv,
OUT/modes.csv and OUT/figures/*.svg. Every number is a simulation result: no sockets, no
loss, simulated time, and no bandwidth or CPU model.

Usage: ablation_table.py OUT
"""

import csv
import os
import sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "testbed"))
import svgplot  # noqa: E402

KS = [1, 3, 5]
MODES = ["full", "static-bucket", "vanilla"]
SUBTITLE = "simulation (no sockets, simulated time, no CPU or bandwidth model); N = 100, mean of rgg and mesh, 3 seeds; equal values overlap"


def num(v):
    return float(v) if v not in ("", None) else None


def mean(values):
    v = [x for x in values if x is not None]
    return sum(v) / len(v) if v else None


def fmt(v, digits=3):
    return "—" if v is None else f"{v:.{digits}g}"


def load(out):
    with open(f"{out}/results.csv") as f:
        rows = list(csv.DictReader(f))
    for r in rows:
        for k in ("k", "q", "intensity", "seed", "N"):
            r[k] = num(r[k])
        r["k"] = int(r["k"])
        r["intensity"] = int(r["intensity"])
    return rows


def metrics(r):
    links = num(r["honest_links_total"]) or 0
    cover = num(r["coverage_links"]) or 0
    deliveries = num(r["honest_deliveries"]) or 0
    return {
        "honest_links_banned": (num(r["honest_links_ever_banned"]) / links) if links else None,
        "honest_nodes_banned": (num(r["honest_nodes_banned"]) / num(r["honest_nodes"])) if num(r["honest_nodes"]) else None,
        "coverage": (num(r["coverage_banned"]) / cover) if cover else None,
        "admitted_per_link": (num(r["attacker_frames_admitted"]) / cover) if cover else None,
        "evict_median_s": (num(r["evict_sim_median_ms"]) / 1000) if r["evict_sim_median_ms"] else None,
        "delivery_attack": num(r["delivery_attack"]),
        "goodput_per_s": num(r["goodput_per_s"]),
        "frames_per_delivery": (num(r["frames"]) / deliveries) if deliveries else None,
        "verifications_per_delivery": (num(r["verifications"]) / deliveries) if deliveries else None,
        "rate_drops": num(r["rate_drops"]),
        "origin_drops": num(r["origin_drops"]),
    }


def group_mean(rows, key):
    by = defaultdict(list)
    for r in rows:
        by[key(r)].append(metrics(r))
    return {k: {m: mean([x[m] for x in v]) for m in v[0]} for k, v in by.items()}, by


def k_criteria(krows, per_run):
    cells, _ = group_mean(krows, lambda r: (r["q"], r["k"]))
    qs = sorted({q for q, _ in cells})
    out = []
    out.append(("at every q, honest links ever banned is non-increasing in k", all(
        cells[(q, 1)]["honest_links_banned"] >= cells[(q, 3)]["honest_links_banned"]
        >= cells[(q, 5)]["honest_links_banned"] for q in qs)))
    out.append(("at every q, attacker frames admitted per link is non-decreasing in k", all(
        cells[(q, 1)]["admitted_per_link"] <= cells[(q, 3)]["admitted_per_link"]
        <= cells[(q, 5)]["admitted_per_link"] for q in qs)))
    out.append(("attacker coverage is 100 % in every run, for every k and q",
                all(num(r["coverage_banned"]) == num(r["coverage_links"]) for r in krows)))
    out.append(("at q = 0 no honest link is banned, for any k",
                all(num(r["honest_links_ever_banned"]) == 0 for r in krows if r["q"] == 0)))
    return out, cells, qs


def mode_criteria(mrows):
    replay = [r for r in mrows if r["attack"] == "replay"]
    return [
        ("full: every replaying attacker banned at every honest neighbour (coverage 100 %)", all(
            num(r["coverage_banned"]) == num(r["coverage_links"]) for r in replay if r["defence"] == "full")),
        ("static-bucket and vanilla: no attacker banned in any run", all(
            num(r["coverage_banned"]) == 0 for r in mrows if r["defence"] != "full")),
        ("full: delivery >= 0.99 against replay at every intensity", all(
            (num(r["delivery_attack"]) or 0) >= 0.99 for r in replay if r["defence"] == "full")),
        ("0 honest false evictions in every modes run", all(num(r["false_evictions"]) == 0 for r in mrows)),
    ]


def main():
    out = sys.argv[1]
    rows = load(out)
    krows = [r for r in rows if r["group"] == "k"]
    mrows = [r for r in rows if r["group"] == "modes"]
    os.makedirs(f"{out}/figures", exist_ok=True)
    lines = []

    if krows:
        kc, cells, qs = k_criteria(krows, None)
        with open(f"{out}/k_tradeoff.csv", "w", newline="") as f:
            w = csv.writer(f, lineterminator="\n")
            names = list(next(iter(cells.values())))
            w.writerow(["q", "k"] + names)
            for q in qs:
                for k in KS:
                    w.writerow([q, k] + [cells[(q, k)][n] for n in names])
        lines += ["## Strike threshold k against honest duplicates (simulation)", "",
                  "| q | k | honest links ever banned | honest nodes banned | attacker coverage | attacker frames admitted per link | eviction median (simulated s) | delivery from the fault |",
                  "|---|---|---|---|---|---|---|---|"]
        for q in qs:
            for k in KS:
                c = cells[(q, k)]
                lines.append(f"| {q:g} | {k} | {fmt(c['honest_links_banned'])} | {fmt(c['honest_nodes_banned'])} | "
                             f"{fmt(c['coverage'])} | {fmt(c['admitted_per_link'])} | {fmt(c['evict_median_s'])} | "
                             f"{fmt(c['delivery_attack'])} |")
        lines += ["", "| criterion | holds |", "|---|---|"]
        lines += [f"| {n} | {'yes' if ok else 'NO'} |" for n, ok in kc]
        labels = [f"{q:g}" for q in qs]
        svgplot.line_chart(
            f"{out}/figures/k-honest-links-banned.svg",
            "Honest links banned against honest duplicates, by strike threshold k", SUBTITLE, labels,
            {f"k = {k}": [cells[(q, k)]["honest_links_banned"] for q in qs] for k in KS},
            "share of honest links ever banned", 0.0, 1.0,
            "probability q that an honest node's frame reaches its peer twice (injected fault)")
        svgplot.line_chart(
            f"{out}/figures/k-attacker-frames-admitted.svg",
            "Attacker frames admitted per link before the ban, by strike threshold k", SUBTITLE,
            [f"k = {k}" for k in KS],
            {"replay x2 attacker (q = 0)": [cells[(0.0, k)]["admitted_per_link"] for k in KS]},
            "frames from the attacker admitted per link", 0.0, None)

    if mrows:
        cells, _ = group_mean(mrows, lambda r: (r["attack"], r["intensity"], r["defence"]))
        with open(f"{out}/modes.csv", "w", newline="") as f:
            w = csv.writer(f, lineterminator="\n")
            names = list(next(iter(cells.values())))
            w.writerow(["attack", "intensity", "defence"] + names)
            for key in sorted(cells, key=lambda k: (k[0], k[1], MODES.index(k[2]))):
                w.writerow(list(key) + [cells[key][n] for n in names])
        lines += ["", "## Defence modes against replay and flood (simulation)", "",
                  "| attack | intensity | mode | delivery from the fault | goodput (deliveries/s) | frames per honest delivery | verifications per honest delivery | attacker coverage | attacker frames admitted per link | rate-limit drops | origin-budget drops |",
                  "|---|---|---|---|---|---|---|---|---|---|---|"]
        for key in sorted(cells, key=lambda k: (k[0], k[1], MODES.index(k[2]))):
            c = cells[key]
            lines.append(f"| {key[0]} | {key[1]} | {key[2]} | {fmt(c['delivery_attack'])} | {fmt(c['goodput_per_s'])} | "
                         f"{fmt(c['frames_per_delivery'])} | {fmt(c['verifications_per_delivery'])} | {fmt(c['coverage'])} | "
                         f"{fmt(c['admitted_per_link'])} | {fmt(c['rate_drops'])} | {fmt(c['origin_drops'])} |")
        lines += ["", "| criterion | holds |", "|---|---|"]
        lines += [f"| {n} | {'yes' if ok else 'NO'} |" for n, ok in mode_criteria(mrows)]
        for attack, xs, xlab in (("replay", [0, 2, 8, 32], "copies of every frame sent by each attacker (0 = no attack)"),
                                 ("flood", [0, 2, 8, 32], "fresh messages per second from each attacker (0 = no attack)")):
            def series(metric):
                result = {}
                for mode in MODES:
                    ys = []
                    for x in xs:
                        key = ("none", 0, mode) if x == 0 else (attack, x, mode)
                        ys.append(cells.get(key, {}).get(metric))
                    result[mode] = ys
                return result
            for metric, ylabel, ymin, ymax in (
                    ("delivery_attack", "delivery of honest messages", 0.0, 1.0),
                    ("frames_per_delivery", "frames processed per honest delivery", 0.0, None)):
                svgplot.line_chart(
                    f"{out}/figures/{metric}-{attack}.svg",
                    f"{ylabel.capitalize()} against {'replay' if attack == 'replay' else 'a fresh-message flood'}",
                    SUBTITLE, [str(x) for x in xs], series(metric), ylabel, ymin, ymax, xlab)

    open(f"{out}/ablation.md", "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""The four evaluation figures, drawn from data committed in this repository only.

  a  dissemination latency against N with 0, 10 and 30 % of the nodes replaying   (emulation)
  b  reputation of a paced attacker over six hours, three replay periods          (simulation)
  c  survivability against packet loss                                            (emulation)
  d  goodput against attack intensity for three defence modes                     (emulation)

Emulation = real crci-node processes in network namespaces on one host (scripts/testbed). The
simulation in (b) is the deterministic in-process benchmark with a simulated clock
(crates/crci-node/benches/recovery_bench.rs); it has no random element, so it has no error bars.
Error bars are the sample standard deviation over the runs behind a point (n is in each subtitle).

Usage: make_figures.py [OUT_DIR]      (default docs/figures)
Standard library only; the plot helper is scripts/testbed/svgplot.py.
"""

import csv
import os
import statistics
import sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "scripts", "testbed"))
import svgplot  # noqa: E402

RATIO = "docs/evidence/ratio-2026-11-07/ratio.csv"
TRAJECTORY = "crates/crci-node/benches/results/recovery_trajectory.csv"
NETEM = "docs/evidence/testbed-fast-recovery-2026-11-04/netem/results.csv"
BASELINES = "docs/evidence/ablation-2026-11-06/testbed/baselines.csv"


def rows(rel):
    with open(os.path.join(ROOT, rel), newline="") as f:
        return list(csv.DictReader(f))


def num(s):
    try:
        return float(s)
    except (TypeError, ValueError):
        return None


def mean_sd(values):
    values = [v for v in values if v is not None]
    if not values:
        return None, None
    return statistics.mean(values), (statistics.stdev(values) if len(values) > 1 else 0.0)


def fig_a(out):
    by = defaultdict(list)
    for r in rows(RATIO):
        if r["f"] in ("0", "10", "30") and r["faithful"] == "True":
            by[(r["kind"], int(r["f"]), int(r["n"]))].append(num(r["lat_p95_post_ms"]))
    sizes = sorted({k[2] for k in by})
    series, errors, counts = {}, {}, set()
    for kind in ("rgg", "mesh"):
        for f in (0, 10, 30):
            stats = [mean_sd(by[(kind, f, n)]) for n in sizes]
            series[f"{kind} {f} %"] = [m for m, _ in stats]
            errors[f"{kind} {f} %"] = [s for _, s in stats]
            counts |= {len([v for v in by[(kind, f, n)] if v is not None]) for n in sizes}
    assert counts == {3}, counts  # three seeds behind every point
    svgplot.line_chart(f"{out}/fig-a-latency-vs-n.svg",
                       "Delivery latency after the quarantine against network size",
                       "emulation (real processes, one host); p95, mean ± sd over 3 seeds; legend: topology, share of nodes replaying",
                       [f"N = {n}" for n in sizes], series, "milliseconds", errors=errors)


def fig_b(out):
    series = defaultdict(list)
    for r in rows(TRAJECTORY):
        series[int(r["period_s"])].append((int(r["t_s"]) / 3600.0, float(r["score"])))
    chosen = {f"replay every {p} s": series[p] for p in (120, 290, 330)}
    xmax = max(x for pts in chosen.values() for x, _ in pts)
    svgplot.xy_chart(f"{out}/fig-b-paced-attack.svg", "Reputation of a paced replayer over six hours",
                     "simulation (deterministic, simulated clock, no variation); ban below 0.1, readmission at 0.25",
                     chosen, "hours", "reputation score", xmax, ymin=0.0, ymax=0.5,
                     hlines=[(0.1, "ban threshold 0.1")])


def fig_c(out):
    by = defaultdict(list)
    for r in rows(NETEM):
        if num(r["reorder_pct"]) == 0 and r["faithful"] == "True":
            by[int(num(r["loss_pct"]))].append(r)
    losses = sorted(by)
    assert losses == [0, 5, 10, 15, 20], losses
    assert {len(v) for v in by.values()} == {18}, {k: len(v) for k, v in by.items()}
    deliv, surv = {}, {}
    for loss, rs in by.items():
        deliv[loss] = mean_sd([num(r["deliv_post"]) for r in rs])
        surv[loss] = mean_sd([1 - num(r["honest_banned"]) / num(r["honest_nodes"]) for r in rs])
        # The replayer is banned by every neighbour in every one of these runs (stated in the docs).
        assert all(r["banned_by"].split("/")[0] == r["banned_by"].split("/")[1] for r in rs), loss
    names = {"delivery after the ban": deliv, "honest nodes not banned": surv}
    svgplot.line_chart(f"{out}/fig-c-survivability-vs-loss.svg", "Survivability against packet loss",
                       "emulation; mean ± sd over 18 runs; replayer banned by all neighbours in every run",
                       [f"{loss} %" for loss in losses],
                       {n: [d[loss][0] for loss in losses] for n, d in names.items()},
                       "share of 1 (axis starts at 0.98)",
                       ymin=0.98, ymax=1.0, xlabel="packet loss per hop (rgg and mesh, N 20/50/100, 3 seeds)",
                       errors={n: [d[loss][1] for loss in losses] for n, d in names.items()})


def fig_d(out):
    order = [("none", "0"), ("replay", "2"), ("replay", "8"), ("replay", "32"),
             ("flood", "40"), ("flood", "100"), ("flood", "200")]
    labels = ["none"] + [f"replay x{i}" for i in (2, 8, 32)] + [f"flood {i}/s" for i in (40, 100, 200)]
    by = defaultdict(list)
    for r in rows(BASELINES):
        if r["faithful"] == "True":
            by[(r["mode"], r["attack"], r["intensity"])].append(num(r["goodput_per_s"]))
    series, errors = {}, {}
    for mode in ("full", "static-bucket", "vanilla"):
        stats = [mean_sd(by.get((mode, a, i), [])) for a, i in order]
        series[mode] = [m for m, _ in stats]
        errors[mode] = [s for _, s in stats]
    svgplot.line_chart(f"{out}/fig-d-goodput-vs-baselines.svg", "Goodput against attack intensity, three defence modes",
                       "emulation, rgg N = 50; mean ± sd over 3 seeds; vanilla at flood 200/s omitted (host saturated)",
                       labels, series, "honest deliveries per second", ymin=0.0, ymax=60.0,
                       xlabel="attack (replay: copies per message; flood: fresh messages per second)",
                       errors=errors)


def main():
    out = os.path.join(ROOT, sys.argv[1] if len(sys.argv) > 1 else "docs/figures")
    os.makedirs(out, exist_ok=True)
    for fig in (fig_a, fig_b, fig_c, fig_d):
        fig(out)
        print("wrote", fig.__name__)


if __name__ == "__main__":
    main()

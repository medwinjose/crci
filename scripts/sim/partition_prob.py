#!/usr/bin/env python3
"""How often do replaying attackers cut the honest nodes apart once they are quarantined?

A graph computation, no protocol and no time: for a topology instance (the generators of
scripts/testbed/topology.py, one instance per seed) a share f of the nodes is chosen as attackers
(never the source; the same draw as scripts/sim/run_sim.py) and removed together with their links,
which is what the quarantine does once every honest neighbour has banned them (the simulation and
the testbed runs verify that it does). The honest nodes are partitioned when the rest is not one
connected component. Reported per topology, size and f: the share of instances partitioned (with
a 95 % Wilson interval), the mean largest component and the mean number of honest nodes cut off
from it.

  --check SIM_RESULTS.csv   compares the per-instance outcome with the simulation's
                            `honest_components` for every run with attackers in that file

Usage: partition_prob.py OUT_DIR [--sizes 100,500,1000] [--seeds 200] [--check results.csv]
Standard library only. Seeds 1..S; the instance depends only on (kind, N, seed).
"""

import argparse
import csv
import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "testbed"))
import svgplot  # noqa: E402
import topology  # noqa: E402

FRACTIONS = [10, 20, 30, 45]


def components(n, adj, removed):
    seen, sizes = set(removed), []
    for s in range(n):
        if s in seen:
            continue
        stack, size = [s], 0
        seen.add(s)
        while stack:
            u = stack.pop()
            size += 1
            for v in adj[u]:
                if v not in seen:
                    seen.add(v)
                    stack.append(v)
        sizes.append(size)
    return sizes


def attackers_of(n, seed, f):
    rng = random.Random(seed * 1000 + f)
    source = rng.randrange(n)
    others = [i for i in range(n) if i != source]
    return sorted(rng.sample(others, round(n * f / 100)))


def instance(kind, n, seed):
    rng = random.Random(seed)
    edges, _ = topology.rgg(n, rng) if kind == "rgg" else topology.regular_mesh(n, rng)
    return topology.adjacency(n, edges)


def wilson(k, n, z=1.96):
    if n == 0:
        return None, None
    p = k / n
    d = 1 + z * z / n
    c = p + z * z / (2 * n)
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return round((c - h) / d, 4), round((c + h) / d, 4)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--sizes", default="100,500,1000")
    ap.add_argument("--seeds", type=int, default=200)
    ap.add_argument("--check", default=None)
    args = ap.parse_args()
    sizes = [int(x) for x in args.sizes.split(",")]
    os.makedirs(f"{args.out}/figures", exist_ok=True)

    per_instance = {}
    rows = []
    for kind in ("rgg", "mesh"):
        for n in sizes:
            adjs = {s: instance(kind, n, s) for s in range(1, args.seeds + 1)}
            for f in FRACTIONS:
                parts, largest, cut = 0, [], []
                for s, adj in adjs.items():
                    gone = attackers_of(n, s, f)
                    sizes_ = components(n, adj, gone)
                    honest = n - len(gone)
                    per_instance[(kind, n, f, s)] = len(sizes_)
                    parts += len(sizes_) > 1
                    largest.append(max(sizes_) / honest)
                    cut.append(honest - max(sizes_))
                lo, hi = wilson(parts, args.seeds)
                rows.append({"topology": kind, "N": n, "f_pct": f, "instances": args.seeds,
                             "partitioned": parts, "p_partitioned": round(parts / args.seeds, 4),
                             "ci95_low": lo, "ci95_high": hi,
                             "mean_largest_component": round(sum(largest) / len(largest), 4),
                             "mean_nodes_cut_off": round(sum(cut) / len(cut), 2)})
                print(rows[-1], flush=True)

    with open(f"{args.out}/partition.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    xl = [f"{f} %" for f in FRACTIONS]
    for kind in ("rgg", "mesh"):
        series = {f"N = {n}": [r["p_partitioned"] for r in rows if r["topology"] == kind and r["N"] == n]
                  for n in sizes}
        svgplot.line_chart(f"{args.out}/figures/partition-probability-{kind}.svg",
                           f"Probability that the quarantine partitions the honest nodes ({kind})",
                           f"graph computation over {args.seeds} topology instances per point; "
                           "attackers removed with their links", xl, series, "share of instances",
                           ymin=0.0, ymax=1.0, xlabel="share of nodes replaying")

    if args.check:
        bad, compared = [], 0
        with open(args.check) as fh:
            for r in csv.DictReader(fh):
                spec = r["attackers_spec"]
                if r["group"] != "scale" or spec in ("one", "0"):
                    continue
                key = (r["kind"], int(r["N"]), int(spec), int(r["seed"]))
                if key not in per_instance:
                    continue
                compared += 1
                if (per_instance[key] > 1) != (int(r["honest_components"]) > 1):
                    bad.append((key, per_instance[key], r["honest_components"]))
        print(f"check against {args.check}: {compared} simulation runs compared, {len(bad)} differ")
        for b in bad:
            print("  differs:", b)
        with open(f"{args.out}/partition-check.txt", "w") as fh:
            fh.write(f"{compared} simulation runs compared, {len(bad)} differ\n")
            for b in bad:
                fh.write(f"differs: {b}\n")


if __name__ == "__main__":
    main()

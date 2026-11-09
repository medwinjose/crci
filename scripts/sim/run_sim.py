#!/usr/bin/env python3
"""Run the in-process mesh simulations and collect one results.csv. Standard library only.

Every run is unmodified NodeRuntime instances without sockets (crates/crci-node/examples/mesh_sim);
this script only writes scenarios from seeds, with the testbed's topology generators and traffic
plan (scripts/testbed/topology.py, testbed.py). Time is simulated: results say nothing about real
latency. Attackers relay honestly until 20 s, then send every message twice; they do not originate
and do not collude.

Groups (each over rgg and the 4-regular mesh, seeds 1 2 3):
  cross  N = 20, 50, 100, one attacker: same topology, roles and plan as the testbed's main runs
  scale  N = 100, 500, 1000, 2000, 5000; one attacker and f = 0, 10, 20, 30, 45 % attackers
  long   mesh N = 300 and rgg N = 150, f = 0 and 10 %, 8 messages/s for 15 min: past the seen-ID
         cache limit (1792), the digest limit (256 origins, mesh only) and the time a banned peer
         needs to be readmitted.

Usage: run_sim.py OUT BIN [--groups cross,scale,long] [--sizes 100,500] [--seeds 1,2,3]
                          [--max-mem-gib 7]
Existing outcomes are kept, so an interrupted sweep resumes with the same arguments.
"""

import argparse
import csv
import json
import os
import random
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "testbed"))
import testbed  # noqa: E402
import topology  # noqa: E402

CROSS_SIZES = [20, 50, 100]
SCALE_SIZES = [100, 500, 1000, 2000, 5000]
FRACTIONS = [0, 10, 20, 30, 45]
# All-pairs distances and the exhaustive role search are too slow in Python above this size.
EXACT_METRICS_MAX_N = 500
PATH_SAMPLES = 64
ROLE_SAMPLES = 16
# Peak memory (measured), for how many runs fit at once.
MIB_PER_NODE = 0.65
LONG_RUN_GIB = 3.4
LONG_SIZES = {"mesh": 300, "rgg": 150}
TOPOLOGY_LOCKS = {}


def build_topology(kind, n, seed, path):
    """The topology as topology.py makes it. Metrics and roles are sampled above
    EXACT_METRICS_MAX_N; the instance depends only on (kind, n, seed) either way."""
    if os.path.exists(path):
        with open(path) as f:
            return json.load(f)
    rng = random.Random(seed)
    edges, params = topology.rgg(n, rng) if kind == "rgg" else topology.regular_mesh(n, rng)
    adj = topology.adjacency(n, edges)
    if n <= EXACT_METRICS_MAX_N:
        dists = [topology.bfs(adj, i) for i in range(n)]
        metrics = topology.metrics(n, edges, adj, dists)
        roles = topology.pick_roles(n, edges, adj, dists)
    else:
        metrics = sampled_metrics(n, edges, adj, seed)
        roles = sampled_roles(n, edges, adj, seed)
    topo = {"kind": kind, "seed": seed, "params": params, "metrics": metrics, "roles": roles,
            "edges": sorted(edges)}
    with open(path + ".tmp", "w") as f:
        json.dump(topo, f)
    os.replace(path + ".tmp", path)
    return topo


def sampled_metrics(n, edges, adj, seed):
    rng = random.Random(seed + 23)
    degrees = [len(a) for a in adj]
    dists = [topology.bfs(adj, s) for s in rng.sample(range(n), PATH_SAMPLES)]
    reached = [d for dist in dists for d in dist if d > 0]
    return {
        "nodes": n,
        "edges": len(edges),
        "mean_degree": round(sum(degrees) / n, 3),
        "min_degree": min(degrees),
        "max_degree": max(degrees),
        "diameter": max(reached),
        "avg_shortest_path": round(sum(reached) / len(reached), 4),
        "clustering": round(topology.clustering(adj), 4),
    }


def sampled_roles(n, edges, adj, seed):
    """pick_roles on a sample: the highest-degree relay whose removal keeps the graph connected,
    and the best of ROLE_SAMPLES sources at least two hops from it."""
    rng = random.Random(seed + 29)
    b = next(v for v in sorted(range(n), key=lambda v: (-len(adj[v]), v))
             if topology.connected(n, edges, removed=v))
    adj_b = topology.adjacency(n, edges, removed=b)
    best = None
    for s in rng.sample(range(n), ROLE_SAMPLES):
        dist = topology.bfs(adj, s)
        if s == b or dist[b] < 2:
            continue
        without = topology.bfs(adj_b, s)
        downstream = [x for x in range(n) if x not in (s, b) and without[x] > dist[x]]
        if best is None or len(downstream) > len(best[1]):
            best = (s, downstream)
    return {"source": best[0], "byzantine": b, "byzantine_degree": len(adj[b]),
            "downstream": best[1]}


def scenario_of(topo, attackers_spec, seed, window, source_rate, bg_rate):
    n = topo["metrics"]["nodes"]
    rng = random.Random(seed * 1000 + (0 if attackers_spec == "one" else attackers_spec))
    if attackers_spec == "one":
        roles = topo["roles"]
        source, attackers, downstream = roles["source"], [roles["byzantine"]], roles["downstream"]
    else:
        source = rng.randrange(n)
        others = [i for i in range(n) if i != source]
        attackers = sorted(rng.sample(others, round(n * attackers_spec / 100)))
        downstream = []
    plan = testbed.plan_traffic(
        n, {"source": source, "byzantine": None, "attackers": attackers},
        window, source_rate, bg_rate, seed)
    phases = random.Random(seed + 11)
    return {
        "n": n,
        "edges": topo["edges"],
        "source": source,
        "attackers": attackers,
        "downstream": downstream,
        "plan": [plan[i] for i in range(n)],
        "sync_phase_ms": [phases.randrange(30000) for _ in range(n)],
        "window_ms": window * 1000,
        "fault_at_ms": 20000,
        "drain_ms": 5000,
        "replay_copies": 2,
        "tick_ms": 10,
    }


def plan_runs(args):
    runs = []
    for seed in args.seeds:
        for kind in ("rgg", "mesh"):
            if "cross" in args.groups:
                for n in CROSS_SIZES:
                    runs.append(("cross", kind, n, seed, "one", 60, 0.5, 0.5))
            if "scale" in args.groups:
                for n in args.sizes or SCALE_SIZES:
                    for spec in ["one"] + FRACTIONS:
                        runs.append(("scale", kind, n, seed, spec, 60, 0.5, 0.5))
            if "long" in args.groups:
                for spec in (0, 10):
                    runs.append(("long", kind, LONG_SIZES[kind], seed, spec, 900, 4, 4))
    return runs


def run_name(run):
    group, kind, n, seed, spec, *_ = run
    return f"{group}-{kind}-{n}-f{spec}-s{seed}"


def run_one(run, args):
    group, kind, n, seed, spec, window, source_rate, bg_rate = run
    name = run_name(run)
    outcome_path = f"{args.out}/outcomes/{name}.json"
    if os.path.exists(outcome_path):
        return
    topo_path = f"{args.out}/topologies/{kind}-{n}-s{seed}.json"
    with TOPOLOGY_LOCKS.setdefault(topo_path, threading.Lock()):
        topo = build_topology(kind, n, seed, topo_path)
    scenario_path = f"{args.out}/{name}.scenario.json"
    with open(scenario_path, "w") as f:
        json.dump(scenario_of(topo, spec, seed, window, source_rate, bg_rate), f)
    started = time.time()
    subprocess.run([args.bin, scenario_path, outcome_path + ".tmp"], check=True,
                   stdout=subprocess.DEVNULL)
    os.replace(outcome_path + ".tmp", outcome_path)
    os.remove(scenario_path)
    print(f"{time.strftime('%H:%M:%S')} {name} {time.time() - started:.1f} s", flush=True)


def collect(args, runs):
    rows = []
    for run in runs:
        name = run_name(run)
        group, kind, n, seed, spec, window, source_rate, bg_rate = run
        path = f"{args.out}/outcomes/{name}.json"
        if not os.path.exists(path):
            continue
        with open(path) as f:
            outcome = json.load(f)
        with open(f"{args.out}/topologies/{kind}-{n}-s{seed}.json") as f:
            m = json.load(f)["metrics"]
        downstream_pre = outcome.pop("downstream_pre")
        downstream_post = outcome.pop("downstream_post")
        rows.append({
            "run": name, "group": group, "kind": kind, "N": n, "seed": seed, "attackers_spec": spec,
            "window_s": window, "load_msgs_per_s": source_rate + bg_rate, "edges": m["edges"],
            "mean_deg": m["mean_degree"], "max_deg": m["max_degree"], "diam": m["diameter"],
            "aspl": m["avg_shortest_path"], "clust": m["clustering"],
            "down_pre": f"{downstream_pre[0]}/{downstream_pre[1]}",
            "down_post": f"{downstream_post[0]}/{downstream_post[1]}",
            **outcome,
        })
    with open(f"{args.out}/results.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]), lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    print(f"{len(rows)} of {len(runs)} runs in {args.out}/results.csv", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("bin")
    ap.add_argument("--groups", default="cross,scale,long")
    ap.add_argument("--sizes", default=None, help="scale group only")
    ap.add_argument("--seeds", default="1,2,3")
    ap.add_argument("--max-mem-gib", type=float, default=7)
    ap.add_argument("--max-workers", type=int, default=os.cpu_count())
    args = ap.parse_args()
    args.groups = args.groups.split(",")
    args.seeds = [int(s) for s in args.seeds.split(",")]
    args.sizes = [int(s) for s in args.sizes.split(",")] if args.sizes else None
    args.bin = os.path.abspath(args.bin)
    os.makedirs(f"{args.out}/outcomes", exist_ok=True)
    os.makedirs(f"{args.out}/topologies", exist_ok=True)

    runs = plan_runs(args)
    # Smallest first; as many at once as fit in the memory limit.
    def key(run):
        return (run[0] == "long", run[2])
    runs.sort(key=key)
    start = 0
    while start < len(runs):
        end = start
        while end < len(runs) and key(runs[end]) == key(runs[start]):
            end += 1
        group, _, n, *_ = runs[start]
        each_gib = LONG_RUN_GIB if group == "long" else n * MIB_PER_NODE / 1024
        workers = max(1, min(args.max_workers, int(args.max_mem_gib / each_gib)))
        with ThreadPoolExecutor(workers) as pool:
            for future in [pool.submit(run_one, run, args) for run in runs[start:end]]:
                future.result()
        collect(args, runs)
        start = end


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Baselines and the strike-threshold ablation in the in-process simulation. Standard library only.

Every run is unmodified NodeRuntime instances without sockets (crates/crci-node/examples/mesh_sim),
as run_sim.py, with the evaluation's switches in the scenario's `config`. Time is simulated: these
are simulation results and say nothing about real latency. N = 100, 10 % attackers, traffic
4 + 4 messages/s, fault at 20 s, topologies and traffic plans from the testbed's generators.

Groups (rgg and 4-regular mesh, seeds 1 2 3):
  k      strike threshold k = 1, 3, 5 against honest duplicates. Honest nodes duplicate an outgoing
         frame with probability q = 0, 1e-5, 3e-5, 1e-4, 3e-4, 1e-3 (an injected transport fault, not a
         measured network property); attackers repeat every frame (replay x2). Window 600 s.
  modes  defence = full, static-bucket, vanilla against: no attack; replay x2, x8, x32; and a flood of
         fresh messages at 2, 8 and 32 per second from each attacker (20, 80 and 320 per second
         in all). Window 60 s.

Usage: run_ablation.py OUT BIN [--groups k,modes] [--seeds 1,2,3] [--max-workers N]
Existing outcomes are kept, so an interrupted sweep resumes with the same arguments.
"""

import argparse
import csv
import json
import os
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run_sim  # noqa: E402

N = 100
ATTACKER_PERCENT = 10
KS = [1, 3, 5]
QS = [0.0, 1e-5, 3e-5, 1e-4, 3e-4, 1e-3]
K_WINDOW_S = 600
MODES = ["full", "static-bucket", "vanilla"]
ATTACKS = [("none", 0), ("replay", 2), ("replay", 8), ("replay", 32),
           ("flood", 2), ("flood", 8), ("flood", 32)]
MODES_WINDOW_S = 60
FAULT_S = 20
MIB_PER_RUN = 120  # measured on the first runs; a conservative bound for how many fit at once
TOPOLOGY_LOCKS = {}


def plan_runs(groups, seeds):
    runs = []
    for seed in seeds:
        for kind in ("rgg", "mesh"):
            if "k" in groups:
                for q in QS:
                    for k in KS:
                        runs.append({"group": "k", "kind": kind, "seed": seed, "k": k, "q": q,
                                     "defence": "full", "attack": "replay", "intensity": 2,
                                     "window": K_WINDOW_S})
            if "modes" in groups:
                for defence in MODES:
                    for attack, intensity in ATTACKS:
                        runs.append({"group": "modes", "kind": kind, "seed": seed, "k": 3, "q": 0.0,
                                     "defence": defence, "attack": attack, "intensity": intensity,
                                     "window": MODES_WINDOW_S})
    return runs


def name_of(r):
    return (f"{r['group']}-{r['kind']}-s{r['seed']}-{r['defence']}-k{r['k']}-q{r['q']:g}"
            f"-{r['attack']}{r['intensity']}")


def scenario_of(topo, r):
    attackers_spec = 0 if r["attack"] == "none" else ATTACKER_PERCENT
    scenario = run_sim.scenario_of(topo, attackers_spec, r["seed"], r["window"], 4, 4)
    scenario["replay_copies"] = r["intensity"] if r["attack"] == "replay" else 1
    if r["attack"] == "flood":
        # Each attacker originates `intensity` fresh messages/s from 10 s before the fault (the
        # testbed's `--byz-rate`), as many as fit in the window.
        start = FAULT_S - 10
        rate = r["intensity"]
        for a in scenario["attackers"]:
            scenario["plan"][a] = (int((r["window"] - start) * rate), max(1, int(1000 / rate)),
                                   start * 1000)
    scenario["config"] = {"defence": r["defence"], "strikes_to_ban": r["k"],
                          "honest_dup_prob": r["q"], "dup_seed": r["seed"] * 7919}
    return scenario


def run_one(r, args):
    name = name_of(r)
    outcome_path = f"{args.out}/outcomes/{name}.json"
    if os.path.exists(outcome_path):
        return
    topo_path = f"{args.out}/topologies/{r['kind']}-{N}-s{r['seed']}.json"
    with TOPOLOGY_LOCKS.setdefault(topo_path, threading.Lock()):
        topo = run_sim.build_topology(r["kind"], N, r["seed"], topo_path)
    scenario_path = f"{args.out}/{name}.scenario.json"
    with open(scenario_path, "w") as f:
        json.dump(scenario_of(topo, r), f)
    started = time.time()
    subprocess.run([args.bin, scenario_path, outcome_path + ".tmp"], check=True,
                   stdout=subprocess.DEVNULL)
    os.replace(outcome_path + ".tmp", outcome_path)
    os.remove(scenario_path)
    print(f"{time.strftime('%H:%M:%S')} {name} {time.time() - started:.1f} s", flush=True)


def collect(args, runs):
    rows = []
    for r in runs:
        path = f"{args.out}/outcomes/{name_of(r)}.json"
        if not os.path.exists(path):
            continue
        with open(path) as f:
            outcome = json.load(f)
        outcome.pop("downstream_pre", None)
        outcome.pop("downstream_post", None)
        rows.append({"run": name_of(r), **r, "N": N, "attackers_pct": 0 if r["attack"] == "none"
                     else ATTACKER_PERCENT, **outcome})
    with open(f"{args.out}/results.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]), lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    print(f"{len(rows)} of {len(runs)} runs in {args.out}/results.csv", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("bin")
    ap.add_argument("--groups", default="k,modes")
    ap.add_argument("--seeds", default="1,2,3")
    ap.add_argument("--max-workers", type=int, default=os.cpu_count())
    args = ap.parse_args()
    args.bin = os.path.abspath(args.bin)
    os.makedirs(f"{args.out}/outcomes", exist_ok=True)
    os.makedirs(f"{args.out}/topologies", exist_ok=True)
    runs = plan_runs(args.groups.split(","), [int(s) for s in args.seeds.split(",")])
    with ThreadPoolExecutor(args.max_workers) as pool:
        for future in [pool.submit(run_one, r, args) for r in runs]:
            future.result()
    collect(args, runs)


if __name__ == "__main__":
    main()

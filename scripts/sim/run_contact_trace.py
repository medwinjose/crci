#!/usr/bin/env python3
"""Replay a human face-to-face contact trace through `mesh_sim`.

Data: SocioPatterns, ACM Hypertext 2009 conference (113 badge wearers, about 2.5 days, 20 s
resolution), CC BY-NC-SA 3.0. L. Isella et al., "What's in a crowd? Analysis of face-to-face
behavioral networks", J. Theor. Biol. 271, 166 (2011). http://www.sociopatterns.org
The file is downloaded to --data (not committed) and checked against a recorded SHA-256.

A link exists only while its two badges are in contact (consecutive 20 s snapshots merge into one
interval). Nodes are people; every node originates a message every 30 minutes; frames cross a
link only while it is up; nodes ask each other for what they miss when a contact starts.

  run_contact_trace.py OUT MESH_SIM [--data DIR] [--seeds 1 2 3] [--attackers-pct 5]
"""
import argparse, gzip, hashlib, json, os, random, subprocess, sys, urllib.request

URL = "http://www.sociopatterns.org/assets/data/ht2009_contact_list.dat.gz"
SHA256 = "43014e65bb8f6aa7d75d36a8ab61279b617bee88197e04a330c4386305ae1738"
STEP = 20  # seconds per snapshot
MESSAGE_EVERY_S = 1800


def load(data_dir):
    path = os.path.join(data_dir, "ht2009_contact_list.dat.gz")
    if not os.path.exists(path):
        os.makedirs(data_dir, exist_ok=True)
        urllib.request.urlretrieve(URL, path)
    raw = open(path, "rb").read()
    if hashlib.sha256(raw).hexdigest() != SHA256:
        sys.exit(f"{path}: SHA-256 differs from the recorded one")
    snaps = {}
    for line in gzip.decompress(raw).decode().splitlines():
        t, a, b = line.split()
        snaps.setdefault((int(a), int(b)), []).append(int(t))
    ids = sorted({x for pair in snaps for x in pair})
    index = {x: i for i, x in enumerate(ids)}
    contacts = []  # (start_ms, end_ms, a, b): interval of one pair, snapshots merged
    for (a, b), times in snaps.items():
        times.sort()
        start = prev = times[0]
        for t in times[1:] + [None]:
            if t is None or t - prev > STEP:
                contacts.append((start - STEP, prev, index[a], index[b]))
                start = t
            prev = t if t is not None else prev
    contacts = [(s * 1000, e * 1000, a, b) for s, e, a, b in contacts]
    return len(ids), sorted(contacts)


def scenario(n, contacts, seed, pct):
    rng = random.Random(seed)
    end_ms = max(e for _, e, _, _ in contacts)
    edges = sorted({(min(a, b), max(a, b)) for _, _, a, b in contacts})
    attackers = sorted(rng.sample(range(n), round(n * pct / 100))) if pct else []
    count = end_ms // 1000 // MESSAGE_EVERY_S
    return {
        "n": n, "edges": edges, "source": None, "attackers": attackers, "downstream": [],
        "plan": [(count - 1, MESSAGE_EVERY_S * 1000, rng.randrange(MESSAGE_EVERY_S) * 1000)
                 for _ in range(n)],
        "sync_phase_ms": [rng.randrange(30000) for _ in range(n)],
        "window_ms": end_ms,
        # With attackers they start replaying a quarter of the way in; without, never.
        "fault_at_ms": end_ms // 4 if attackers else end_ms,
        "drain_ms": 5000, "replay_copies": 2, "tick_ms": 1000, "contacts": contacts,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("mesh_sim")
    ap.add_argument("--data", default="data/ht2009")
    ap.add_argument("--seeds", type=int, nargs="+", default=[1, 2, 3])
    ap.add_argument("--attackers-pct", type=float, default=5)
    args = ap.parse_args()
    n, contacts = load(args.data)
    os.makedirs(args.out, exist_ok=True)
    print(f"{n} nodes, {len(contacts)} contact intervals, "
          f"{len({(a, b) for _, _, a, b in contacts})} pairs")
    for label, pct in (("honest", 0), (f"attack{args.attackers_pct:g}", args.attackers_pct)):
        for seed in args.seeds:
            name = f"{label}-s{seed}"
            sc = os.path.join(args.out, name + "-scenario.json")
            oc = os.path.join(args.out, name + "-outcome.json")
            json.dump(scenario(n, contacts, seed, pct), open(sc, "w"))
            subprocess.run([args.mesh_sim, sc, oc], check=True, stdout=subprocess.DEVNULL)
            o = json.load(open(oc))
            print(name, {k: o[k] for k in (
                "honest_nodes", "attackers", "frames", "messages", "false_evictions",
                "honest_strikes", "honest_links_total", "honest_links_ever_banned",
                "coverage_links", "coverage_banned", "attacker_strikes", "sync_requests",
                "sync_excused", "verifications", "runtime_s")},
                  {k: o.get(k) for k in ("coverage_links_in_contact", "coverage_banned_in_contact")})
            os.remove(sc)


main()

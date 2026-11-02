#!/usr/bin/env python3
"""Analyse one testbed run (the OUT directory of testbed.py).

Writes OUT/summary.json, OUT/latency.csv and OUT/bans.csv, and prints the
summary. Periods are by origination time: "warmup" for the first 5 s of the
window (persistent connections are opened by the first messages), "pre"
before the fault starts, "post" after every neighbour of the relay B has
banned it; messages in between are reported as "during".

With impairments the configured round trip is the distribution of two egress delays (uniform in
delay +- jitter, zero for the reordered share), computed by Monte Carlo; the RTT median must lie within
+-10 % and the p95 within +25 % of that distribution's median and p95. Packet loss does not change the
configured RTT of the packets that arrive. Fidelity criteria: every node listening at T0, mean host CPU busy < 70 %
over the window, no swap-in or swap-out and no kernel backlog drops during
the window, and ICMP RTT on sampled overlay links with median <= 1.10x and
p95 <= 1.25x the configured round trip (2 x netem delay). The CPU, swap and
RTT criteria were fixed before any run; the listening and kernel-drop
criteria were added after a 200-node run lost nodes to an ARP storm at
start-up.

Usage: analyze.py OUT
"""

import csv
import json
import random
import re
import sys
from collections import defaultdict, deque

ORIGINATED = re.compile(r"Node (n\d+) originated '([^']+)' at_ms=(\d+)")
ACCEPTED = re.compile(r"Node (n\d+) accepted '([^']+)' \(origin (\S+), via ([^#]+)#\S*\) at_ms=(\d+)")
BANNED = re.compile(r"Node (n\d+) banned ([^#]+)#\S+ at_ms=(\d+)")
READMITTED = re.compile(r"Node (n\d+) readmitted ([^#]+)#\S+ at_ms=(\d+)")
FAULT = re.compile(r"Node (n\d+) fault: .* at_ms=(\d+)")
RATE = re.compile(r"\[(n\d+)\] SYBIL GUARD DROP: peer ([^#]+)#\S+ exceeded rate limit")
RTT = re.compile(r"time=([\d.]+) ms")
WARMUP_MS = 5000


def pct(values, p):
    if not values:
        return None
    s = sorted(values)
    return s[min(len(s) - 1, max(0, int(round(p / 100 * len(s) + 0.5)) - 1))]


def stats(values):
    if not values:
        return {"n": 0}
    return {
        "n": len(values),
        "median": pct(values, 50),
        "p95": pct(values, 95),
        "max": max(values),
        "mean": round(sum(values) / len(values), 2),
    }


def expected_rtt(delay, jitter, reorder, seed=1, samples=200000):
    """Median and p95 of the configured round trip: two egress delays, each uniform in
    delay +- jitter, or zero for the reordered share of packets (netem reorder)."""
    rng = random.Random(seed)

    def one():
        if reorder and rng.random() < reorder / 100:
            return 0.0
        return rng.uniform(delay - jitter, delay + jitter) if jitter else float(delay)

    rtts = [one() + one() for _ in range(samples)]
    return pct(rtts, 50), pct(rtts, 95)


def bfs(adj, src):
    dist = {src: 0}
    q = deque([src])
    while q:
        u = q.popleft()
        for v in adj[u]:
            if v not in dist:
                dist[v] = dist[u] + 1
                q.append(v)
    return dist


def main():
    out = sys.argv[1]
    with open(f"{out}/meta.json") as f:
        meta = json.load(f)
    topo = meta["topology"]
    n = topo["metrics"]["nodes"]
    roles = topo["roles"]
    name = lambda i: f"n{i:03d}"
    src, byz = name(roles["source"]), name(roles["byzantine"])
    downstream = {name(x) for x in roles["downstream"]}
    cfg = meta["config"]
    delay = cfg["delay_ms"]
    jitter, loss, reorder = cfg.get("jitter_ms", 0), cfg.get("loss", 0), cfg.get("reorder", 0)
    # Mean one-way delay per hop (the reordered share skips the delay).
    mean_hop = delay * (1 - reorder / 100)
    t0, end = meta["t0_ms"], meta["end_ms"]

    # Topology hop counts from every node (for latency per hop).
    with open(f"{out}/topo_edges.json") as f:
        edges = json.load(f)
    adj = defaultdict(set)
    for a, b in edges:
        adj[name(a)].add(name(b))
        adj[name(b)].add(name(a))
    hops = {name(i): bfs(adj, name(i)) for i in range(n)}

    originated, accepted, bans, readmits = {}, defaultdict(dict), [], []
    via = []
    rate_drops = defaultdict(int)
    fault_ms = None
    counters = defaultdict(int)
    for i in range(n):
        with open(f"{out}/logs/n{i}.log", errors="replace") as f:
            for line in f:
                if m := ORIGINATED.search(line):
                    originated[m[2]] = (m[1], int(m[3]))
                elif m := ACCEPTED.search(line):
                    accepted[m[2]].setdefault(m[1], (int(m[5]), m[4], m[3]))
                    via.append((m[1], m[2], m[4], int(m[5])))
                elif m := BANNED.search(line):
                    bans.append((m[1], m[2], int(m[3])))
                elif m := READMITTED.search(line):
                    readmits.append((m[1], m[2], int(m[3])))
                elif m := FAULT.search(line):
                    fault_ms = int(m[2])
                elif m := RATE.search(line):
                    rate_drops["from_byzantine" if m[2] == byz else "from_honest"] += 1
                if "received message from" in line:
                    counters["frames_received"] += 1
                    if "id=head-" in line:
                        counters["chain_head_frames"] += 1
                for key in ("MALICIOUS REPLAY", "REPLAY dropped", "INVALID SIGNATURE",
                            "STALE", "PUBKEY MISMATCH", "is banned"):
                    if key in line:
                        counters[key] += 1

    honest = [name(i) for i in range(n) if name(i) != byz]
    byz_neighbours = sorted(adj[byz])
    byz_bans = {obs: t for obs, banned, t in bans if banned == byz}
    false_evictions = [(o, b, t) for o, b, t in bans if b != byz]
    all_banned = len(byz_bans) == len(byz_neighbours) and byz_bans
    post_from = max(byz_bans.values()) if all_banned else None

    def period(t_emit):
        # Persistent connections open on first use: the first seconds include handshakes.
        if t_emit < t0 + WARMUP_MS:
            return "warmup"
        if fault_ms is None or t_emit < fault_ms:
            return "pre"
        if post_from is not None and t_emit >= post_from:
            return "post"
        return "during"

    rows = []
    delivery = defaultdict(list)
    downstream_delivery = defaultdict(list)
    for mid, (origin, t_emit) in originated.items():
        if not (t0 - 1000 <= t_emit <= end):
            continue
        p = period(t_emit)
        receivers = [x for x in honest if x != origin]
        got = accepted.get(mid, {})
        delivery[p].append(sum(1 for x in receivers if x in got) / len(receivers))
        if origin == src:
            for x in downstream:
                downstream_delivery[p].append(1 if x in got else 0)
        for x, (t_acc, sender, _) in got.items():
            if x == byz or x == origin:
                continue
            h = hops[origin].get(x)
            rows.append((mid, origin, x, h, t_acc - t_emit, p, x in downstream and origin == src))

    with open(f"{out}/latency.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["message", "origin", "receiver", "hops", "latency_ms", "period", "downstream_of_byzantine"])
        w.writerows(rows)
    with open(f"{out}/bans.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["observer", "banned", "at_ms", "since_fault_ms"])
        for o, b, t in sorted(bans, key=lambda r: r[2]):
            w.writerow([o, b, t, t - fault_ms if fault_ms else ""])

    lat = defaultdict(list)
    per_hop_excess = []
    down_lat = defaultdict(list)
    for mid, origin, x, h, l, p, is_down in rows:
        lat[p].append(l)
        if h:
            per_hop_excess.append(round((l - h * mean_hop) / h, 2))
        if is_down:
            down_lat[p].append(l)
    via_byz = defaultdict(int)
    for x, mid, sender, t in via:
        if sender == byz and mid in originated:
            via_byz[period(originated[mid][1])] += 1

    # Host and RSS.
    host = []
    with open(f"{out}/host.csv") as f:
        for r in csv.DictReader(f):
            if t0 <= int(r["t_ms"]) <= end:
                host.append(r)
    cpu = [float(r["cpu_busy_pct"]) for r in host]
    swap_io = (int(host[-1]["pswpin"]) - int(host[0]["pswpin"]) +
               int(host[-1]["pswpout"]) - int(host[0]["pswpout"])) if host else None
    kernel_drops = (int(host[-1]["softnet_drops"]) - int(host[0]["softnet_drops"])) if host else None
    rss = []
    with open(f"{out}/rss.csv") as f:
        for r in csv.DictReader(f):
            if int(r["rss_kib"]) > 0:
                rss.append(int(r["rss_kib"]))

    rtts = []
    for a, b in meta["ping_edges"]:
        with open(f"{out}/ping/{a}-{b}.txt") as f:
            rtts += [float(x) for x in RTT.findall(f.read())]
    rtt = stats(rtts)
    exp_med, exp_p95 = expected_rtt(delay, jitter, reorder)
    fidelity = {
        "all_nodes_listening_at_t0": meta["listening_at_t0"] == n,
        "cpu_mean_below_70": bool(cpu) and sum(cpu) / len(cpu) < 70,
        "no_kernel_backlog_drops": kernel_drops == 0,
        "no_swap_io": swap_io == 0,
        "rtt_median_within_10pct": rtt.get("median") is not None
        and 0.90 * exp_med <= rtt["median"] <= 1.10 * exp_med,
        "rtt_p95_within_25pct": rtt.get("p95") is not None and rtt["p95"] <= 1.25 * exp_p95,
    }
    fidelity["pass"] = all(fidelity.values())

    summary = {
        "run": out,
        "topology": {k: topo[k] for k in ("kind", "seed", "metrics")},
        "config": meta["config"],
        "launch": {"listening_at_t0": meta["listening_at_t0"], "alive_at_end": meta["alive_at_end"],
                   "nodes": n},
        "roles": {"source": src, "byzantine": byz, "byzantine_degree": len(byz_neighbours),
                  "downstream": len(downstream)},
        "messages_originated_in_window": sum(1 for _, t in originated.values() if t0 - 1000 <= t <= end),
        "delivery_ratio": {p: stats(v) for p, v in delivery.items()},
        "messages_fully_delivered": {p: sum(1 for d in v if d == 1.0) for p, v in delivery.items()},
        "latency_ms": {p: stats(v) for p, v in lat.items()},
        "per_hop_excess_ms": stats(per_hop_excess),
        "downstream_delivery": {p: (sum(v), len(v)) for p, v in downstream_delivery.items()},
        "downstream_latency_ms": {p: stats(v) for p, v in down_lat.items()},
        "accepted_via_byzantine": dict(via_byz),
        "fault_ms_since_t0": fault_ms - t0 if fault_ms else None,
        "byzantine_banned_by": f"{len(byz_bans)}/{len(byz_neighbours)}",
        "eviction_latency_ms": stats([t - fault_ms for t in byz_bans.values()]) if fault_ms else None,
        "honest_false_evictions": len(false_evictions),
        "honest_nodes_banned_by_anyone": len({b for _, b, _ in false_evictions}),
        "honest_nodes": len(honest),
        "readmissions": len(readmits),
        "rate_limit_drops": dict(rate_drops),
        "log_counters": dict(counters),
        "frames_received_per_message": round(
            counters["frames_received"] / max(1, len(originated)), 2),
        "host": {"cpu_busy_pct": stats(cpu),
                 "mem_used_mib_max": max(int(r["mem_used_mib"]) for r in host) if host else None,
                 "swap_used_mib_max": max(int(r["swap_used_mib"]) for r in host) if host else None,
                 "swap_pages_in_out": swap_io,
                 "kernel_backlog_drops": kernel_drops},
        "rss_kib_per_node": stats(rss),
        "rss_total_mib": round(sum(rss) / 1024, 1),
        "ping_rtt_ms": rtt,
        "configured_rtt_ms": {"median": exp_med, "p95": exp_p95},
        "fidelity": fidelity,
    }
    with open(f"{out}/summary.json", "w") as f:
        json.dump(summary, f, indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()

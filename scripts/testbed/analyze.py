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
ACCEPTED = re.compile(r"Node (n\d+) accepted '([^']+)' \(origin (\S+), via ([^#\s)]+)(?:#\S*)?\) at_ms=(\d+)"
                      r"(?: path=(\w+))?(?: proc_us=(\d+))?(?: \(\+(\d+) not printed\))?")
RECEIVED = re.compile(r"Node (n\d+) received message from ([^#:\s]+)(?:#\S*)?:.*?"
                      r"(?: \(\+(\d+) not printed\))?$")
RELAY_DROPS = re.compile(r"Node (n\d+) relay drops: failed=(\d+) queue_full=(\d+)")
BANNED = re.compile(r"Node (n\d+) banned ([^#\s]+)(?:#\S*)? at_ms=(\d+)")
READMITTED = re.compile(r"Node (n\d+) readmitted ([^#\s]+)(?:#\S*)? at_ms=(\d+)")
FAULT = re.compile(r"Node (n\d+) fault: .* at_ms=(\d+)")
RATE = re.compile(r"\[(n\d+)\] SYBIL GUARD DROP: peer ([^#\s]+)(?:#\S*)? exceeded rate limit"
                  r"(?: \(\+(\d+) not printed\))?")
ORIGIN_DROP = re.compile(r"\[(n\d+)\] ORIGIN BUDGET DROP: '[^']+' from origin '([^']+)'.*?"
                         r"(?: \(\+(\d+) not printed\))?$")
RTT = re.compile(r"time=([\d.]+) ms")
PACED = re.compile(r"Node (n\d+) frames paced: total=(\d+)")
BANNED_DROPPED = re.compile(r"Node (n\d+) banned-peer frames dropped: total=(\d+)")
LINES_HELD = re.compile(r"Node (n\d+) log lines not printed: received=(\d+) accepted=(\d+)")
SYNC_REQUESTED = re.compile(r"Node (n\d+) requested sync from ")
WARMUP_MS = 5000
# Each node prints its label and its key-derived node ID once at start; every other line names
# peers and origins by node ID. (Logs from builds before key-derived IDs name peers `label#key`;
# the patterns accept both.)
NODE_ID = re.compile(r"^Node (\S+) id ([0-9a-f]{32})$")
HEX_ID = re.compile(r"\b[0-9a-f]{32}\b")


def node_labels(paths):
    """Node ID -> label, from the start-up line of each log."""
    labels = {}
    for path in paths:
        with open(path, errors="replace") as f:
            for line in f:
                if m := NODE_ID.match(line.strip()):
                    labels[m[2]] = m[1]
                    break
    return labels


def labelled(line, labels):
    """`line` with every node ID replaced by its node's label."""
    return HEX_ID.sub(lambda m: labels.get(m[0], m[0]), line) if labels else line


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
    origin_drops = defaultdict(int)
    relay_drops = {}
    banned_dropped = {}
    paced_frames = {}
    proc_us = []
    fault_ms = None
    counters = defaultdict(int)
    # Lines a node held back (its log limit): the cumulative totals it prints every 5 s and at
    # shutdown, or, for the accepted lines, the counts at the end of later lines if larger.
    held_total = {}
    accepted_suffix = defaultdict(int)
    labels = node_labels([f"{out}/logs/n{i}.log" for i in range(n)])
    for i in range(n):
        with open(f"{out}/logs/n{i}.log", errors="replace") as f:
            for line in f:
                line = labelled(line, labels)
                if m := ORIGINATED.search(line):
                    originated[m[2]] = (m[1], int(m[3]))
                elif m := ACCEPTED.search(line):
                    accepted[m[2]].setdefault(m[1], (int(m[5]), m[4], m[3], m[6] or "gossip"))
                    via.append((m[1], m[2], m[4], int(m[5])))
                    if m[7]:
                        proc_us.append(int(m[7]))
                    accepted_suffix[m[1]] += int(m[8] or 0)
                elif m := BANNED.search(line):
                    bans.append((m[1], m[2], int(m[3])))
                elif m := READMITTED.search(line):
                    readmits.append((m[1], m[2], int(m[3])))
                elif m := FAULT.search(line):
                    fault_ms = int(m[2])
                elif m := RELAY_DROPS.search(line):
                    relay_drops[m[1]] = (int(m[2]), int(m[3]))  # cumulative: keep the last
                elif m := PACED.search(line):
                    paced_frames[m[1]] = int(m[2])  # cumulative: keep the last
                elif m := LINES_HELD.search(line):
                    held_total[m[1]] = (int(m[2]), int(m[3]))  # cumulative: keep the last
                elif m := BANNED_DROPPED.search(line):
                    banned_dropped[m[1]] = int(m[2])  # cumulative: keep the last
                elif m := RATE.search(line):
                    rate_drops["from_byzantine" if m[2] == byz else "from_honest"] += 1 + int(m[3] or 0)
                elif m := ORIGIN_DROP.search(line):
                    origin_drops["byzantine_origin" if m[2] == byz else "honest_origin"] += 1 + int(m[3] or 0)
                if SYNC_REQUESTED.search(line):
                    counters["sync_requests"] += 1
                if m := RECEIVED.search(line):
                    counters["frames_received"] += 1
                    # The overlay is enforced by configuration only (one shared L2 segment):
                    # count frames from nodes that are not overlay neighbours.
                    if m[2] not in adj[m[1]]:
                        counters["frames_from_non_neighbours"] += 1 + int(m[3] or 0)
                    if "id=head-" in line:
                        counters["chain_head_frames"] += 1
                for key in ("MALICIOUS REPLAY", "REPLAY dropped", "INVALID SIGNATURE",
                            "ORIGIN NOT DERIVED FROM KEY", "STALE", "is banned"):
                    if key in line:
                        counters[key] += 1

    counters["frames_received"] += sum(received for received, _ in held_total.values())
    counters["accepted_lines_not_printed"] = sum(
        max(held_total.get(node, (0, 0))[1], accepted_suffix[node])
        for node in set(held_total) | set(accepted_suffix))

    honest = [name(i) for i in range(n) if name(i) != byz]
    byz_neighbours = sorted(adj[byz])
    byz_bans = {obs: t for obs, banned, t in bans if banned == byz}
    # With several attackers (--attackers-pct) only the first is `byz`; a ban of any attacker is not false.
    attacker_names = {name(a) for a in roles.get("attackers", [roles["byzantine"]])} | {byz}
    false_evictions = [(o, b, t) for o, b, t in bans if b not in attacker_names]
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
        # Delivery and latency describe honest traffic; B's own messages are meant to die with it.
        if origin == byz or not (t0 - 1000 <= t_emit <= end):
            continue
        p = period(t_emit)
        receivers = [x for x in honest if x != origin]
        got = accepted.get(mid, {})
        delivery[p].append(sum(1 for x in receivers if x in got) / len(receivers))
        if origin == src:
            for x in downstream:
                downstream_delivery[p].append(1 if x in got else 0)
        for x, (t_acc, sender, _, path) in got.items():
            if x == byz or x == origin:
                continue
            h = hops[origin].get(x)
            rows.append((mid, origin, x, h, t_acc - t_emit, p, x in downstream and origin == src, path))

    # Delivery and goodput of honest messages originated from the fault (the configured fault time
    # when no fault was injected) to the end of the window, whether or not anyone was banned: the
    # baselines never ban, so the "post" period does not exist for them.
    attack_from = fault_ms if fault_ms else t0 + meta["config"].get("fault_at", 20) * 1000
    attack_ratios, attack_deliveries = [], 0
    for mid, (origin, t_emit) in originated.items():
        if origin == byz or not (attack_from <= t_emit <= end):
            continue
        receivers = [x for x in honest if x != origin]
        got = accepted.get(mid, {})
        delivered = sum(1 for x in receivers if x in got)
        attack_ratios.append(delivered / len(receivers))
        attack_deliveries += delivered
    attack_seconds = max(1.0, (end - attack_from) / 1000)

    with open(f"{out}/latency.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["message", "origin", "receiver", "hops", "latency_ms", "period", "downstream_of_byzantine",
                    "path"])
        w.writerows(rows)
    with open(f"{out}/bans.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["observer", "banned", "at_ms", "since_fault_ms"])
        for o, b, t in sorted(bans, key=lambda r: r[2]):
            w.writerow([o, b, t, t - fault_ms if fault_ms else ""])

    lat = defaultdict(list)
    per_hop_excess = []
    down_lat = defaultdict(list)
    late = defaultdict(int)  # deliveries more than 5 s after origination, by path
    by_path = defaultdict(int)
    for mid, origin, x, h, l, p, is_down, path in rows:
        by_path[path] += 1
        if l > 5000:
            late[path] += 1
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
    cpu_by_node = {}
    with open(f"{out}/rss.csv") as f:
        for r in csv.DictReader(f):
            if int(r["rss_kib"]) > 0:
                rss.append(int(r["rss_kib"]))
            if float(r.get("cpu_s", -1)) >= 0:
                cpu_by_node[name(int(r["node"]))] = float(r["cpu_s"])
    # Process CPU seconds over the whole run (utime + stime), by role.
    byz_nbrs = set(adj[byz])
    cpu_roles = {
        "byzantine": cpu_by_node.get(byz),
        "byzantine_neighbours_mean": stats([v for k, v in cpu_by_node.items() if k in byz_nbrs])
        .get("mean"),
        "byzantine_neighbours_max": stats([v for k, v in cpu_by_node.items() if k in byz_nbrs])
        .get("max"),
        "others_mean": stats([v for k, v in cpu_by_node.items() if k not in byz_nbrs and k != byz])
        .get("mean"),
    }

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
        # A node above its per-second line limit holds acceptance lines back: delivery would be
        # undercounted.
        "every_acceptance_logged": counters["accepted_lines_not_printed"] == 0,
    }
    fidelity["pass"] = all(fidelity.values())

    # Link flapping (testbed.py --flap-fraction): the judgement of a flapping run counts every ban
    # and every strike on any node, since without a fault no node deserves either, and delivery to
    # every node from every origin, since links are up again for the last 20 s of the window and the drain.
    flap = None
    if cfg.get("flap_fraction") or cfg.get("no_fault"):
        applied = 0
        try:
            with open(f"{out}/flaps.csv") as f:
                applied = max(0, sum(1 for _ in f) - 1)
        except FileNotFoundError:
            pass
        everyone = [name(i) for i in range(n)]
        ratios, missing = [], 0
        for mid, (origin, t_emit) in originated.items():
            if not (t0 - 1000 <= t_emit <= end):
                continue
            got = accepted.get(mid, {})
            receivers = [x for x in everyone if x != origin]
            delivered = sum(1 for x in receivers if x in got)
            ratios.append(delivered / len(receivers))
            missing += len(receivers) - delivered
        flap = {
            "events_planned": meta.get("flap_events", 0),
            "events_applied": applied,
            "bans_total": len(bans),
            "bans": sorted({f"{o}->{x}" for o, x, _ in bans})[:20],
            "strikes_total": counters["MALICIOUS REPLAY"] + counters["REPLAY dropped"]
            + counters["INVALID SIGNATURE"],
            "sync_requests": counters["sync_requests"],
            "messages": len(ratios),
            "delivery_all_nodes": stats(ratios),
            "messages_fully_delivered": sum(1 for r in ratios if r == 1.0),
            "undelivered_pairs": missing,
            "latency_all_ms": stats([l for _, _, _, _, l, _, _, _ in rows]),
        }
        fidelity_extra = {"flap_events_all_applied": applied == meta.get("flap_events", 0)}
    else:
        fidelity_extra = {}

    # Several attackers (testbed.py --attackers-pct): every attacker replays from the fault on and
    # none colludes. Bans are judged between honest observers only (an attacker's opinion of any node
    # is not evidence), delivery is judged between honest nodes, and the honest nodes' graph without
    # the attackers says how much of the mesh the quarantine leaves connected.
    ratio = None
    if cfg.get("attackers_pct"):
        bad = {name(a) for a in roles["attackers"]}
        good = [name(i) for i in range(n) if name(i) not in bad]
        good_set = set(good)
        pairs = {(o, b): t for o, b, t in bans if o in good_set and b in bad}
        expected = {(o, a) for a in bad for o in adj[a] if o in good_set}
        false_pairs = {(o, b) for o, b, _ in bans if o in good_set and b in good_set}
        # Components of the honest nodes' graph (attackers removed).
        gadj = {u: adj[u] & good_set for u in good}
        comp, comps = {}, []
        for v in good:
            if v in comp:
                continue
            members = list(bfs(gadj, v))
            for x in members:
                comp[x] = len(comps)
            comps.append(members)
        times = [pairs[p] for p in expected if p in pairs]
        done = max(times) if expected and len(times) == len(expected) else None
        by_period = defaultdict(list)       # delivery to every honest node
        by_period_comp = defaultdict(list)  # delivery inside the origin's honest component
        lat_by_period = defaultdict(list)
        for mid, (origin, t_emit) in originated.items():
            if origin not in good_set or not (t0 - 1000 <= t_emit <= end):
                continue
            if t_emit < t0 + WARMUP_MS:
                continue
            if fault_ms is None or t_emit < fault_ms:
                p = "pre"
            elif done is not None and t_emit >= done:
                p = "post"
            else:
                p = "during"
            got = accepted.get(mid, {})
            recv = [x for x in good if x != origin]
            if not recv:
                continue
            by_period[p].append(sum(1 for x in recv if x in got) / len(recv))
            inside = [x for x in recv if comp[x] == comp[origin]]
            if inside:
                by_period_comp[p].append(sum(1 for x in inside if x in got) / len(inside))
            for x in recv:
                if x in got:
                    lat_by_period[p].append(got[x][0] - t_emit)
        ratio = {
            "attackers": len(bad),
            "honest": len(good),
            "attacker_honest_neighbour_pairs": len(expected),
            "pairs_banned": len(expected & set(pairs)),
            "coverage": round(len(expected & set(pairs)) / len(expected), 4) if expected else None,
            "attackers_without_honest_neighbour": sum(1 for a in bad if not (adj[a] & good_set)),
            "attackers_fully_quarantined": sum(
                1 for a in bad if (adj[a] & good_set) and all((o, a) in pairs for o in adj[a] & good_set)),
            "honest_false_evictions": len(false_pairs),
            "eviction_latency_ms": stats([t - fault_ms for t in times]) if fault_ms and times else None,
            "quarantine_done_ms_since_fault": (done - fault_ms) if done and fault_ms else None,
            "components": len(comps),
            "largest_component_fraction": round(max(len(c) for c in comps) / len(good), 4),
            "delivery_all_honest": {p: stats(v) for p, v in by_period.items()},
            "delivery_in_component": {p: stats(v) for p, v in by_period_comp.items()},
            "latency_ms": {p: stats(v) for p, v in lat_by_period.items()},
        }

    if fidelity_extra:
        fidelity.update(fidelity_extra)
        fidelity["pass"] = all(v for k, v in fidelity.items() if k != "pass")

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
        "attack_window": {"messages": len(attack_ratios), "delivery": stats(attack_ratios),
                          "honest_deliveries": attack_deliveries,
                          "goodput_per_s": round(attack_deliveries / attack_seconds, 2),
                          "seconds": round(attack_seconds, 1)},
        "messages_fully_delivered": {p: sum(1 for d in v if d == 1.0) for p, v in delivery.items()},
        "latency_ms": {p: stats(v) for p, v in lat.items()},
        "per_hop_excess_ms": stats(per_hop_excess),
        # Protocol compute per delivering hop, measured inside crci-node (excludes the frame
        # signature check, about 37 us); the rest of the per-hop excess is host and kernel time.
        "protocol_compute_us_per_hop": stats(proc_us),
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
        "banned_frames_dropped": sum(banned_dropped.values()),
        "frames_paced": {"total": sum(paced_frames.values()), "nodes": sorted(paced_frames)},
        "rate_limit_drops": dict(rate_drops),
        "origin_budget_drops": dict(origin_drops),
        "relay_drops": {"failed": sum(f for f, _ in relay_drops.values()),
                        "queue_full": sum(q for _, q in relay_drops.values()),
                        "nodes_with_drops": len(relay_drops)},
        "deliveries_by_path": dict(by_path),
        "late_deliveries_over_5s_by_path": dict(late),
        "log_counters": dict(counters),
        "frames_from_non_neighbours": counters["frames_from_non_neighbours"],
        "frames_received_per_message": round(
            counters["frames_received"] / max(1, len(originated)), 2),
        "host": {"cpu_busy_pct": stats(cpu),
                 "mem_used_mib_max": max(int(r["mem_used_mib"]) for r in host) if host else None,
                 "swap_used_mib_max": max(int(r["swap_used_mib"]) for r in host) if host else None,
                 "swap_pages_in_out": swap_io,
                 "kernel_backlog_drops": kernel_drops},
        "rss_kib_per_node": stats(rss),
        "cpu_seconds_per_process": cpu_roles,
        "rss_total_mib": round(sum(rss) / 1024, 1),
        "ping_rtt_ms": rtt,
        "configured_rtt_ms": {"median": exp_med, "p95": exp_p95},
        "fidelity": fidelity,
        "flap": flap,
        "ratio": ratio,
    }
    with open(f"{out}/summary.json", "w") as f:
        json.dump(summary, f, indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()

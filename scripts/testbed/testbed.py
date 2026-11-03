#!/usr/bin/env python3
"""Run one emulation experiment: real crci-node processes, one per Linux
network namespace, on one bridge, with netem delay on every node's link.

The topology (from topology.py) is the overlay: each node is given its
neighbours with --peers and talks to no one else; the bridge only provides
the shared emulated link layer. Run as root on Linux (tested in WSL2).

Traffic during the measurement window (WINDOW seconds from T0):
  - the source S originates SOURCE_RATE signed messages per second;
  - the other honest nodes originate BG_RATE messages per second in total,
    round-robin over a seeded permutation;
  - the relay B relays honestly until T0 + FAULT_AT, then sends every gossip
    message REPLAY_COPIES times (default 2: one repeat; crci-node
    --replay-after-ms, --replay-copies), so its neighbours strike it.

Recorded in OUT: logs/n<i>.log (node stdout), host.csv (CPU, memory, swap
every second), ping/<a>-<b>.txt (ICMP RTT on sampled overlay links during
the window), rss.csv (per-node resident memory at the end of the window)
and meta.json (configuration, timings, roles, addresses).

Usage: testbed.py --topo T.json --out DIR --bin crci-node [options]
"""

import argparse
import json
import math
import os
import random
import signal
import subprocess
import threading
import time

NS = "cb"
BRIDGE = "crcibr"
PORT = 7000
BACKLOG = "/proc/sys/net/core/netdev_max_backlog"


def sh(*cmd, check=True):
    return subprocess.run(cmd, check=check, stdout=subprocess.DEVNULL,
                          stderr=subprocess.PIPE, text=True)


def ip_of(i):
    return f"10.20.{i // 250}.{i % 250 + 1}"


def mac_of(i):
    return f"02:00:0a:14:{i // 250:02x}:{i % 250 + 1:02x}"


def cleanup():
    out = subprocess.run(["ip", "netns", "list"], capture_output=True, text=True).stdout
    for line in out.splitlines():
        name = line.split()[0] if line.split() else ""
        if name.startswith(NS) and name[len(NS):].isdigit():
            sh("ip", "netns", "del", name, check=False)
    sh("ip", "link", "del", BRIDGE, check=False)
    # A veth whose namespace is already gone can survive; it would block the next run.
    links = subprocess.run(["ip", "-o", "link"], capture_output=True, text=True).stdout
    for line in links.splitlines():
        name = line.split(":")[1].strip().split("@")[0] if ":" in line else ""
        if name.startswith("vh") and name[2:].isdigit():
            sh("ip", "link", "del", name, check=False)


def batch(cmd, lines):
    subprocess.run(cmd + ["-batch", "-"], input="".join(lines), check=True, text=True,
                   stdout=subprocess.DEVNULL)


def netem_args(delay_ms, jitter_ms, loss_pct, reorder_pct):
    """tc netem options for one node's egress. Jitter is uniform in +-jitter_ms around the delay;
    reordering sends that percentage of packets without delay (netem needs a delay for it)."""
    args = ["delay", f"{delay_ms}ms"]
    if jitter_ms:
        args.append(f"{jitter_ms}ms")
    if loss_pct:
        args += ["loss", f"{loss_pct}%"]
    if reorder_pct:
        args += ["reorder", f"{reorder_pct}%"]
    return args + ["limit", "100000"]


def setup_network(n, netem, nbrs):
    sh("ip", "link", "add", BRIDGE, "type", "bridge")
    sh("ip", "link", "set", BRIDGE, "up")
    for i in range(n):
        ns = f"{NS}{i}"
        sh("ip", "netns", "add", ns)
        sh("ip", "link", "add", f"vh{i}", "type", "veth", "peer", "name", "eth0",
           "address", mac_of(i), "netns", ns)
        sh("ip", "link", "set", f"vh{i}", "master", BRIDGE, "up")
        sh("ip", "-n", ns, "addr", "add", f"{ip_of(i)}/16", "dev", "eth0")
        sh("ip", "-n", ns, "link", "set", "eth0", "up")
        sh("ip", "-n", ns, "link", "set", "lo", "up")
        # One-way delay on the node's egress: every overlay hop crosses exactly one.
        sh("tc", "-n", ns, "qdisc", "add", "dev", "eth0", "root", "netem", *netem)
    # Static neighbour and bridge entries: no ARP broadcast or unknown-unicast flooding, which
    # at start-up (every node resolving its neighbours at once) overflows the kernel backlog.
    for i in range(n):
        batch(["ip", "-n", f"{NS}{i}"],
              [f"neigh replace {ip_of(j)} lladdr {mac_of(j)} dev eth0 nud permanent\n"
               for j in nbrs[i]])
    batch(["bridge"], [f"fdb replace {mac_of(i)} dev vh{i} master static\n" for i in range(n)])


def softnet_drops():
    with open("/proc/net/softnet_stat") as f:
        return sum(int(line.split()[1], 16) for line in f)


def read_host():
    with open("/proc/stat") as f:
        cpu = [int(x) for x in f.readline().split()[1:]]
    idle = cpu[3] + cpu[4]
    total = sum(cpu[:8])
    mem = {}
    with open("/proc/meminfo") as f:
        for line in f:
            k, v = line.split(":")
            mem[k] = int(v.split()[0])
    vm = {}
    with open("/proc/vmstat") as f:
        for line in f:
            k, v = line.split()
            vm[k] = int(v)
    return total, idle, mem, vm


def sampler(path, stop):
    with open(path, "w") as out:
        out.write("t_ms,cpu_busy_pct,mem_used_mib,mem_available_mib,swap_used_mib,pswpin,pswpout,"
                  "softnet_drops\n")
        prev_total, prev_idle, _, _ = read_host()
        while not stop.wait(1.0):
            total, idle, mem, vm = read_host()
            dt = total - prev_total
            busy = 100.0 * (dt - (idle - prev_idle)) / dt if dt else 0.0
            prev_total, prev_idle = total, idle
            used = (mem["MemTotal"] - mem["MemAvailable"]) // 1024
            swap = (mem["SwapTotal"] - mem["SwapFree"]) // 1024
            out.write(f"{int(time.time() * 1000)},{busy:.1f},{used},"
                      f"{mem['MemAvailable'] // 1024},{swap},{vm['pswpin']},{vm['pswpout']},"
                      f"{softnet_drops()}\n")
            out.flush()


def plan_traffic(n, roles, window, source_rate, bg_rate, seed):
    """Per node: (count, interval_ms, offset_ms from T0)."""
    s, b = roles["source"], roles["byzantine"]
    plan = {i: (0, 1000, 0) for i in range(n)}
    plan[s] = (int(window * source_rate), int(1000 / source_rate), 0)
    others = [i for i in range(n) if i not in (s, b)]
    random.Random(seed).shuffle(others)
    total = int(window * bg_rate)
    k = len(others)
    for j, node in enumerate(others):
        if j >= total:
            break
        count = math.ceil((total - j) / k)
        plan[node] = (count, int(1000 * k / bg_rate), int(1000 * j / bg_rate))
    return plan


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--topo", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--delay-ms", type=int, default=10)
    ap.add_argument("--jitter-ms", type=int, default=0, help="uniform +- jitter around the delay")
    ap.add_argument("--loss", type=float, default=0, help="packet loss, percent per egress")
    ap.add_argument("--reorder", type=float, default=0,
                    help="percent of packets sent without delay (reordered)")
    ap.add_argument("--window", type=int, default=60)
    ap.add_argument("--source-rate", type=float, default=0.5)
    ap.add_argument("--bg-rate", type=float, default=0.5)
    ap.add_argument("--fault-at", type=int, default=20)
    ap.add_argument("--no-fault", action="store_true")
    ap.add_argument("--replay-copies", type=int, default=2)
    ap.add_argument("--byz-rate", type=float, default=0,
                    help="messages/s the Byzantine node originates from the fault on, to keep "
                         "its neighbours' budget for it empty while it replays")
    ap.add_argument("--refill", type=float, default=None,
                    help="per-neighbour budget refill (messages/s); default: the protocol's")
    ap.add_argument("--settle", type=int, default=20)
    ap.add_argument("--drain", type=int, default=5)
    ap.add_argument("--pings", type=int, default=10)
    ap.add_argument("--worker-threads", type=int, default=2)
    ap.add_argument("--backlog", type=int, default=30000,
                    help="net.core.netdev_max_backlog for the run (restored afterwards)")
    args = ap.parse_args()

    with open(args.topo) as f:
        topo = json.load(f)
    n = topo["metrics"]["nodes"]
    roles = topo["roles"]
    nbrs = {i: [] for i in range(n)}
    for a, b in topo["edges"]:
        nbrs[a].append(b)
        nbrs[b].append(a)

    os.makedirs(f"{args.out}/logs", exist_ok=True)
    os.makedirs(f"{args.out}/ping", exist_ok=True)
    with open(f"{args.out}/topo_edges.json", "w") as f:
        json.dump(topo["edges"], f)

    cleanup()
    # Every message is a burst of one frame per directed overlay link, and on one host all of
    # them pass through the kernel's per-CPU input backlog; separate devices would not share
    # it. Raise it for the run (drops are still counted, and any drop fails the run).
    with open(BACKLOG) as f:
        old_backlog = f.read().strip()
    with open(BACKLOG, "w") as f:
        f.write(str(args.backlog))
    t_setup = time.time()
    setup_network(n, netem_args(args.delay_ms, args.jitter_ms, args.loss, args.reorder), nbrs)
    setup_s = time.time() - t_setup
    print(f"network: {n} namespaces up in {setup_s:.1f} s", flush=True)

    stop = threading.Event()
    host_thread = threading.Thread(target=sampler, args=(f"{args.out}/host.csv", stop))
    host_thread.start()

    plan = plan_traffic(n, roles, args.window, args.source_rate, args.bg_rate, topo["seed"])
    if args.byz_rate and not args.no_fault:
        plan[roles["byzantine"]] = (int((args.window - args.fault_at) * args.byz_rate),
                                    int(1000 / args.byz_rate), args.fault_at * 1000)
    # T0 leaves room for launching every process plus a settling period.
    t0 = time.time() + 0.05 * n + args.settle
    env = dict(os.environ, TOKIO_WORKER_THREADS=str(args.worker_threads))
    procs = []
    t_launch = time.time()
    # Every schedule counts from T0 (namespaces share the host clock), not from each process's
    # start, which `ip netns exec` delays by seconds when hundreds start at once.
    for i in range(n):
        count, interval, offset = plan[i]
        cmd = ["ip", "netns", "exec", f"{NS}{i}", args.bin,
               "--listen", f"{ip_of(i)}:{PORT}", "--node-id", f"n{i:03d}",
               "--peers", ",".join(f"{ip_of(j)}:{PORT}" for j in sorted(nbrs[i])),
               "--experiment-epoch-unix-ms", str(int(t0 * 1000))]
        if count:
            cmd += ["--emit-count", str(count), "--emit-interval-ms", str(interval),
                    "--emit-delay-ms", str(offset)]
        if i == roles["byzantine"] and not args.no_fault:
            cmd += ["--replay-after-ms", str(args.fault_at * 1000),
                    "--replay-copies", str(args.replay_copies)]
        if args.refill is not None:
            cmd += ["--sender-refill-per-sec", str(args.refill)]
        log = open(f"{args.out}/logs/n{i}.log", "w")
        procs.append(subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT, env=env))
        log.close()
    launch_s = time.time() - t_launch
    print(f"launched {n} nodes in {launch_s:.1f} s; T0 in {t0 - time.time():.1f} s", flush=True)
    if time.time() > t0 - 5:
        stop.set()
        raise SystemExit("launch overran the settling period; increase --settle")

    # Every node must be listening before T0.
    while time.time() < t0:
        time.sleep(0.5)
    listening = 0
    for i in range(n):
        with open(f"{args.out}/logs/n{i}.log", errors="replace") as f:
            if "listening on" in f.read():
                listening += 1
    print(f"T0: {listening}/{n} nodes listening", flush=True)

    # ICMP RTT on sampled overlay links while traffic runs.
    rng = random.Random(topo["seed"] + 7)
    edges = [tuple(e) for e in topo["edges"]]
    sample = rng.sample(edges, min(args.pings, len(edges)))
    count = max(1, (args.window - 10) * 2)
    pingers = []
    for a, b in sample:
        out = open(f"{args.out}/ping/{a}-{b}.txt", "w")
        pingers.append(subprocess.Popen(
            ["ip", "netns", "exec", f"{NS}{a}", "ping", "-c", str(count), "-i", "0.5", ip_of(b)],
            stdout=out, stderr=subprocess.STDOUT))
        out.close()

    end = t0 + args.window + args.drain
    while time.time() < end:
        time.sleep(0.5)
    for p in pingers:
        p.wait()

    with open(f"{args.out}/rss.csv", "w") as out:
        out.write("node,pid,rss_kib,threads\n")
        for i, p in enumerate(procs):
            rss = threads = -1
            try:
                with open(f"/proc/{p.pid}/status") as f:
                    for line in f:
                        if line.startswith("VmRSS:"):
                            rss = int(line.split()[1])
                        elif line.startswith("Threads:"):
                            threads = int(line.split()[1])
            except FileNotFoundError:
                pass
            out.write(f"{i},{p.pid},{rss},{threads}\n")
    alive = sum(1 for p in procs if p.poll() is None)

    for p in procs:
        p.send_signal(signal.SIGINT)
    time.sleep(2)
    for p in procs:
        if p.poll() is None:
            p.kill()
        p.wait()
    stop.set()
    host_thread.join()
    cleanup()
    with open(BACKLOG, "w") as f:
        f.write(old_backlog)

    meta = {
        "topology": {k: topo[k] for k in ("kind", "seed", "params", "metrics", "roles")},
        "config": {k: getattr(args, k) for k in (
            "delay_ms", "jitter_ms", "loss", "reorder", "window", "source_rate", "bg_rate", "fault_at", "no_fault", "replay_copies",
            "byz_rate", "refill", "settle", "drain", "pings", "worker_threads", "backlog")},
        "t0_ms": int(t0 * 1000),
        "fault_ms": None if args.no_fault else int((t0 + args.fault_at) * 1000),
        "end_ms": int((t0 + args.window) * 1000),
        "network_setup_s": round(setup_s, 1),
        "launch_s": round(launch_s, 1),
        "listening_at_t0": listening,
        "alive_at_end": alive,
        "ping_edges": sample,
        "plan": {str(i): plan[i] for i in range(n)},
        "addresses": {str(i): f"{ip_of(i)}:{PORT}" for i in range(n)},
    }
    with open(f"{args.out}/meta.json", "w") as f:
        json.dump(meta, f, indent=1)
    print(f"done: {alive}/{n} alive at the end of the window; output in {args.out}", flush=True)


if __name__ == "__main__":
    main()

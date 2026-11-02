# Emulation testbed

Real `crci-node` processes on an emulated network, all on one Linux host. Each node runs in its own
network namespace with one veth link to a shared bridge; `tc netem` adds a one-way delay on every node's
egress, so each overlay hop crosses exactly one delay. Every namespace has a fixed MAC address, static
neighbour entries for its overlay peers and a static bridge forwarding entry, so there is no ARP or
unknown-unicast flooding (when hundreds of nodes resolve their neighbours at once, flooding overflows the
kernel's input backlog). The kernel's per-CPU input backlog (`net.core.netdev_max_backlog`) is raised to
30 000 for the run and restored afterwards: every message is a burst of one frame per directed overlay
link, all through one host's kernel, which separate devices would not share. The topology is the
**overlay**: every node is given its neighbours with `--peers` and talks to no one else. Results are from processes emulated on one host,
not from separate machines or radios.

Linux only, run as root. Tested in WSL2 (Ubuntu 26.04, kernel 6.6, iproute2 6.19). Needs Python 3
(standard library only), `ip`, `tc` and `ping`.

## Files

| File | What it does |
|---|---|
| `topology.py KIND N SEED OUT.json` | Generates a topology and prints its metrics (mean/min/max degree, diameter, average shortest-path length, mean local clustering coefficient). `rgg`: random geometric graph in the unit square, radius `sqrt(2 ln N / (pi N))`, resampled until connected. `mesh`: random 4-regular graph, resampled until simple and connected (diameter 4–6 at N = 20–100). Also picks the roles for the eviction experiment (below). |
| `testbed.py --topo T.json --out DIR --bin crci-node` | Creates the namespaces, starts one node per namespace (`TOKIO_WORKER_THREADS=2`, listening on the namespace's own address), drives the traffic, samples host CPU / memory / swap / kernel backlog drops every second, pings sampled overlay links, records per-node RSS, then stops everything and removes the namespaces. Every node's schedule counts from T0 (`--experiment-epoch-unix-ms`), because `ip netns exec` delays process start by seconds when hundreds of nodes start at once. |
| `analyze.py DIR` | Computes delivery ratio, dissemination latency (overall, per hop, for the downstream set), bans, eviction latency, honest false evictions, rate-limit drops, host load and link RTT, and checks the fidelity criteria. Writes `summary.json`, `latency.csv`, `bans.csv`. |
| `report.py OUT` | Collects every run's `summary.json` into `results.csv`. |
| `run_all.sh OUT [BIN]` | The full experiment set (see the header of the script). |
| `run_netem.sh OUT [BIN] [SEEDS] [SIZES]` | The impairment set: one-way delay 40 ms � 15 ms (uniform) on every egress, with loss 0/5/10/15/20 %, reordering 5/10/15 % and loss 10 % + reordering 10 %; rgg and mesh, N = 20/50/100, seeds 1�3. Resumable. |
| `netem_table.py OUT` | One CSV per condition, `summary.csv` (mean over seeds) and `table.md` under `OUT/netem/`. |

```bash
make testbed OUT=/tmp/crci-testbed
# or: cargo build --release -p crci-node --bin crci-node
#     sudo scripts/testbed/run_all.sh /tmp/crci-testbed target/release/crci-node
```

## Experiment

Each run uses a 60 s measurement window starting at T0, after every node is listening:

- **Traffic:** a source S originates 0.5 signed messages/s; the other honest nodes originate 0.5 messages/s
  in total (round-robin over a seeded permutation). Every node relays every new message to all its
  neighbours (`crci-node --emit-count/--emit-interval-ms/--emit-delay-ms`).
- **Fault:** a relay B behaves honestly until T0 + 20 s, then sends every gossip message twice, i.e.
  repeats each message once (`crci-node --replay-after-ms`, `--replay-copies`). Its neighbours strike it
  and ban it.
- **Roles:** B is a node whose removal keeps the graph connected, at least two hops from S, chosen (with S)
  to maximise the downstream set D: the nodes all of whose shortest paths from S pass through B. After B is
  evicted, S's messages must reach D over longer alternate paths.

Node log lines used by the analysis carry a Unix-millisecond timestamp (`at_ms=`): `originated`,
`accepted`, `banned`, `readmitted` and the fault start. All namespaces share the host clock.

## Fidelity criteria

A run counts as faithful to the configured network only if every node is listening at T0 and, over the
window: mean host CPU busy is below 70 %, there is no swap-in or swap-out, the kernel drops no packets
from its input backlog, and the ICMP round trip on sampled overlay links has a median within 10 % and a
95th percentile within 25 % of the configured round trip (2 × netem delay). The CPU, swap and RTT
criteria were fixed before the first run; the listening and backlog criteria were added after a first
200–500-node pass lost nodes and packets to start-up ARP flooding.

## Known limits

- One host: CPU scheduling and the shared bridge are part of every measurement; the fidelity criteria bound,
  but do not remove, that effect. The median latency above the configured delay per hop grows with N
  (about 1 ms up to N = 100, 6.8 ms at N = 500 in the recorded runs), and protocol cost cannot be
  separated from host contention.
- `run_all.sh` uses a fixed delay; `run_netem.sh` adds loss, jitter and reordering (below). No link failures yet.
- The overlay runs on one shared link layer; nodes are kept to their neighbours by configuration, not by
  filtering.
- `crci-node` logs relay failures through the `log` crate, which the binary does not print, so failed relays
  are not counted.

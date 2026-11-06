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
| `analyze.py DIR` | Computes delivery ratio and dissemination latency of honest-originated messages (overall, per hop, for the downstream set), bans, eviction latency, honest false evictions, rate-limit drops (honest senders' frames separately), per-origin budget drops, relay frames lost, deliveries more than 5 s late by path, host load and link RTT, and checks the fidelity criteria. Writes `summary.json`, `latency.csv`, `bans.csv`. |
| `report.py OUT` | Collects every run's `summary.json` into `results.csv`. |
| `main_table.py OUT` | The main table of `docs/benchmarks.md` (one row per configuration, ranges over seeds) from `OUT/results.csv`. |
| `run_all.sh OUT [BIN]` | The full experiment set (see the header of the script). |
| `run_netem.sh OUT [BIN] [SEEDS] [SIZES]` | The impairment set: one-way delay 40 ms ± 15 ms (uniform) on every egress, with loss 0/5/10/15/20 %, reordering 5/10/15 % and loss 10 % + reordering 10 %; rgg and mesh, N = 20/50/100, seeds 1–3. Resumable. |
| `netem_table.py OUT` | One CSV per condition, `summary.csv` (mean over seeds) and `table.md` under `OUT/netem/`, and `conditions.md`: each condition aggregated over all its runs. |
| `run_attacks.sh OUT [BIN] [SEEDS] [OLD_BIN]` | The attacker set on rgg N = 50: an over-budget replayer (B empties its budget with fresh messages from 10 s before the fault, then sends every frame 8 times; refill 5 and 20, and an older binary for comparison if given), a fresh-message flood (B originates 40 messages/s and never repeats a frame), and the 20 % loss condition with the nodes kept running 60 s after the last message (`--drain 60`). Resumable. |
| `run_flood.sh OUT [BIN] [SEEDS]` | The invalid-signature flood on rgg N = 50: B silent, B sending 40 valid messages/s (the load-matched reference), B forging every signature at 1, 8 and 64 copies per frame (40, 320, 2 560 forged frames/s per link), B flooding 1 000 valid fresh messages/s, and one 480 s run in which B forges for 10 s and then signs correctly (readmission). Resumable. |
| `flood_table.py OUT [BEFORE_DIR]` | `flood.csv`, `flood.md` (means over seeds and the pass/fail criteria) and `figures/flood-cost.svg`; with a directory of runs from an earlier binary, `figures/flood-cpu.svg` compares the CPU of B's neighbours. |

```bash
make testbed OUT=/tmp/crci-testbed
make attacks OUT=/tmp/crci-testbed   # writes /tmp/crci-testbed-attacks
make flood OUT=/tmp/crci-testbed     # writes /tmp/crci-testbed-flood
make tables OUT=/tmp/crci-testbed    # main table and netem tables from runs already there
make smoke OUT=/tmp/crci-testbed     # seed 1, N = 20 netem runs only, to check the scripts
make reproduce OUT=/tmp/crci-testbed # all runs (about 8.5 h), then the tables
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
- **Attacker options:** `--byz-rate R` makes B originate R messages/s of its own from 10 s before the
  fault; `--replay-copies 1` turns the fault into a pure fresh-message flood; `--refill` sets the
  per-neighbour budget refill; `--drain S` keeps the nodes running S seconds after the window.
- **Invalid-signature flood:** `--corrupt-signatures` makes B corrupt the signature of everything it sends
  from the fault on (with `--replay-copies N`, each frame N times); `--corrupt-for S` stops after S seconds
  so B signs correctly again (readmission). `run_flood.sh` runs the set (B silent, B sending valid messages
  at the same rate, 1/8/64 copies, one 480 s readmission run) and `flood_table.py` writes `flood.csv`,
  `flood.md` and the figures. The analysis also records process CPU seconds per role (`/proc/<pid>/stat`).
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
  (0.7–1.2 ms up to N = 100, 2.9–3.2 ms at N = 500 in the recorded runs), while the protocol's own compute
  per accepting hop, logged by each node as `proc_us`, stays at a median of 0.11–0.17 ms at every N; the rest
  is host cost. `proc_us` includes the node's own logging, so it is an upper bound.
- Loss, jitter and reordering are applied per egress by netem, independently of the TCP connections; burst
  loss, asymmetric loss and link failures are not emulated.
- The overlay runs on one shared link layer; nodes are kept to their neighbours by configuration, not by
  filtering. The analysis counts frames received from nodes that are not overlay neighbours
  (`frames_from_non_neighbours` in `summary.json`): 0 in every run of 2026-11-03.
- The kernel's netem randomness is not seeded, so a run with the same seed gives the same topology, roles
  and schedule but not the same packet losses.

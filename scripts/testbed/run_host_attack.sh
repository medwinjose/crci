#!/usr/bin/env bash
# An adversary host with free identities, against nodes that charge their limits per source host
# (the default) and against the same nodes charging them per connection and identity only
# (--per-host-limits off, the behaviour before per-host limits).
#
#   sudo scripts/testbed/run_host_attack.sh OUT_DIR [crci-node] [host_attack] ["seeds"] [kind] [n]
#
# Linux only; run as root. Builds nothing. One-way delay 40 ms +- 15 ms on every egress, 1 message/s
# in total, window 90 s, drain 60 s. The adversary host (examples/host_attack, namespace cbx) is no
# overlay node; B relays honestly. Conditions:
#   ref          no adversary
#   outage       no adversary; the neighbours of B drop every packet they send from 20 s to 30 s
#                after T0, so every connection into them aborts and is opened again
#   hold         as outage, plus the adversary holding idle authenticated connections to every
#                node from 13 addresses (8 per address and node, reopened when closed) from 15 s
#                before T0
#   forge        the adversary sends broken signatures to each neighbour of B from one address from
#                20 s, 8 connections at once (400 frames/s each), under a fresh identity after every ban
#   flood        the adversary sends 100 fresh, validly signed messages/s to B from one address
#                from 20 s, each from a new origin, switching identity every 20
# Every adversary condition runs with --per-host-limits on (suffix -on) and off (-off). Existing
# runs (a summary.json) are skipped, so an interrupted sweep resumes.
set -euo pipefail

OUT=${1:?usage: run_host_attack.sh OUT_DIR [crci-node] [host_attack] [seeds] [kind] [n]}
BIN=${2:-target/release/crci-node}
ATTACK_BIN=${3:-target/release/examples/host_attack}
SEEDS=${4:-"1 2 3"}
KIND=${5:-rgg}
N=${6:-50}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
sha256sum "$BIN" "$ATTACK_BIN" > "$OUT/binary.sha256"

COMMON="--delay-ms 40 --jitter-ms 15 --window 90 --drain 60 --no-fault --host-attack-bin $ATTACK_BIN"
OUTAGE="--outage-at 20 --outage-for 10 --outage-nodes byzantine-neighbours"
HOLD="$OUTAGE --host-attack hold --host-attack-targets all --host-attack-addresses 13 --host-attack-per-source 8 --host-attack-start -15"
FORGE="--host-attack forge --host-attack-per-source 8 --host-attack-rate 400 --host-attack-start 20"
FLOOD="--host-attack flood --host-attack-targets byzantine --host-attack-rate 100 --host-attack-rotate-every 20 --host-attack-start 20"

run() {
    local name=$1 seed=$2
    shift 2
    local topo="$OUT/topo-$KIND-$N-s$seed.json"
    local dir="$OUT/$name-$KIND-$N-s$seed"
    if [ -f "$dir/summary.json" ]; then echo "skip $dir"; return; fi
    [ -f "$topo" ] || python3 "$HERE/topology.py" "$KIND" "$N" "$seed" "$topo" > "$OUT/topo-$KIND-$N-s$seed.txt"
    echo "=== $name $KIND N=$N seed=$seed ==="
    # Pages parked in swap by an earlier run would fail the no-swap criterion.
    swapoff -a 2>/dev/null && swapon -a 2>/dev/null || true
    # shellcheck disable=SC2086
    python3 "$HERE/testbed.py" --topo "$topo" --out "$dir" --bin "$BIN" $COMMON "$@"
    python3 "$HERE/analyze.py" "$dir" > /dev/null
}

for seed in $SEEDS; do
    run ref "$seed"
    # shellcheck disable=SC2086
    run outage "$seed" $OUTAGE
    for limits in on off; do
        # shellcheck disable=SC2086
        run "hold-$limits" "$seed" $HOLD --per-host-limits "$limits"
        # shellcheck disable=SC2086
        run "forge-$limits" "$seed" $FORGE --per-host-limits "$limits"
        # shellcheck disable=SC2086
        run "flood-$limits" "$seed" $FLOOD --per-host-limits "$limits"
    done
done
python3 "$HERE/host_attack_table.py" "$OUT"

#!/usr/bin/env bash
# Run the emulation testbed experiments and analyse each run.
#
#   sudo scripts/testbed/run_all.sh OUT_DIR [crci-node binary] ["seeds"]
#
# Linux only (network namespaces, netem); run as root. Builds nothing: pass a
# release binary, e.g. target/release/crci-node from
#   cargo build --release -p crci-node --bin crci-node
#
# Every configuration runs once per seed (default 1 2 3; the seed selects the topology
# instance, the roles and the traffic schedule). Runs use 10 ms one-way netem delay per
# hop and a 60 s window. Unless stated, traffic is 1 message/s in total (0.5/s from the
# source S, 0.5/s from the other nodes) and the relay B repeats every message once from
# 20 s on.
#   main     rgg and mesh at N = 20, 50, 100
#   load     rgg N = 50 at 4, 6 and 8 messages/s in total (8 is the design load)
#   sweep    rgg at N = 200, 300, 500 (N = 100 is in "main")
#   stress   rgg N = 500 at 4 messages/s
# Existing runs (a summary.json in the run directory) are skipped, so an interrupted
# sweep can be resumed with the same arguments.
set -euo pipefail

OUT=${1:?usage: run_all.sh OUT_DIR [crci-node binary] [seeds]}
BIN=${2:-target/release/crci-node}
SEEDS=${3:-"1 2 3"}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"

run() {
    local name=$1 kind=$2 n=$3 seed=$4
    shift 4
    local dir="$OUT/$name-s$seed"
    if [ -f "$dir/summary.json" ]; then echo "skip $dir"; return; fi
    echo "=== $name ($kind, N=$n, seed $seed) ==="
    python3 "$HERE/topology.py" "$kind" "$n" "$seed" "$dir.topo.json" | tee "$dir.topology.txt"
    python3 "$HERE/testbed.py" --topo "$dir.topo.json" --out "$dir" --bin "$BIN" "$@"
    python3 "$HERE/analyze.py" "$dir" > /dev/null
    echo "summary: $dir/summary.json"
}

for seed in $SEEDS; do
    for kind in rgg mesh; do
        for n in 20 50 100; do
            run "$kind-$n" "$kind" "$n" "$seed"
        done
    done
    run load4-rgg-50 rgg 50 "$seed" --source-rate 2 --bg-rate 2
    run load6-rgg-50 rgg 50 "$seed" --source-rate 3 --bg-rate 3
    run load8-rgg-50 rgg 50 "$seed" --source-rate 4 --bg-rate 4
    for n in 200 300 500; do
        run "sweep-rgg-$n" rgg "$n" "$seed"
    done
    run stress4-rgg-500 rgg 500 "$seed" --source-rate 2 --bg-rate 2
done
python3 "$HERE/report.py" "$OUT"

#!/usr/bin/env bash
# Run the emulation testbed experiments and analyse each run.
#
#   sudo scripts/testbed/run_all.sh OUT_DIR [crci-node binary]
#
# Linux only (network namespaces, netem); run as root. Builds nothing: pass a
# release binary, e.g. target/release/crci-node from
#   cargo build --release -p crci-node --bin crci-node
#
# All runs use seed 1, 10 ms one-way netem delay per hop and a 60 s window.
# Unless stated, traffic is 1 message/s in total (0.5/s from the source S,
# 0.5/s from the other nodes) and the relay B repeats every message once
# from 20 s on.
#   main     rgg and mesh at N = 20, 50, 100
#   load     rgg N = 50 at 4 and 6 messages/s in total (the per-neighbour
#            budget is 5 frames/s)
#   sweep    rgg at N = 200, 300, 500 (N = 100 is in "main")
#   stress   rgg N = 500 at 4 messages/s
set -euo pipefail

OUT=${1:?usage: run_all.sh OUT_DIR [crci-node binary]}
BIN=${2:-target/release/crci-node}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"

run() {
    local name=$1 kind=$2 n=$3
    shift 3
    echo "=== $name ($kind, N=$n) ==="
    python3 "$HERE/topology.py" "$kind" "$n" 1 "$OUT/$name.topo.json" | tee "$OUT/$name.topology.txt"
    python3 "$HERE/testbed.py" --topo "$OUT/$name.topo.json" --out "$OUT/$name" --bin "$BIN" "$@"
    python3 "$HERE/analyze.py" "$OUT/$name" > /dev/null
    echo "summary: $OUT/$name/summary.json"
}

for kind in rgg mesh; do
    for n in 20 50 100; do
        run "$kind-$n" "$kind" "$n"
    done
done
run load4-rgg-50 rgg 50 --source-rate 2 --bg-rate 2
run load6-rgg-50 rgg 50 --source-rate 3 --bg-rate 3
for n in 200 300 500; do
    run "sweep-rgg-$n" rgg "$n"
done
run stress4-rgg-500 rgg 500 --source-rate 2 --bg-rate 2
python3 "$HERE/report.py" "$OUT"

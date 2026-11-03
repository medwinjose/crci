#!/usr/bin/env bash
# Run the attacker and eventual-delivery experiments and analyse each run.
#
#   sudo scripts/testbed/run_attacks.sh OUT_DIR [crci-node binary] ["seeds"] [old crci-node binary]
#
# Linux only (network namespaces, netem); run as root. Builds nothing. Every
# configuration runs once per seed (default 1 2 3) on rgg N = 50 unless stated.
#   over-budget  B originates its own messages from 10 s before the fault at twice the
#                per-neighbour refill rate, so its budget is empty when it starts sending
#                every frame 8 times: refill 5 (B at 10/s) and the default 20 (B at 40/s).
#                With an old binary as fourth argument it also runs that binary at refill 5,
#                for a before/after comparison.
#   flood        B originates 40 fresh messages/s from 10 s before the fault and never
#                repeats a frame (no replay to strike).
#   eventual     the 20 % loss netem condition (rgg and mesh, N = 20, 50, 100) with the
#                nodes kept running 60 s after the last message, so late recoveries count.
# Existing runs (a summary.json in the run directory) are skipped.
set -euo pipefail

OUT=${1:?usage: run_attacks.sh OUT_DIR [crci-node binary] [seeds] [old binary]}
BIN=${2:-target/release/crci-node}
SEEDS=${3:-"1 2 3"}
OLD=${4:-}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT/over-budget" "$OUT/flood" "$OUT/eventual"

run() {
    local dir=$1 kind=$2 n=$3 seed=$4
    shift 4
    if [ -f "$dir/summary.json" ]; then echo "skip $dir"; return; fi
    echo "=== $(basename "$dir") ($kind, N=$n, seed $seed) ==="
    python3 "$HERE/topology.py" "$kind" "$n" "$seed" "$dir.topo.json" | tee "$dir.topology.txt"
    python3 "$HERE/testbed.py" --topo "$dir.topo.json" --out "$dir" "$@"
    python3 "$HERE/analyze.py" "$dir" > /dev/null
}

for seed in $SEEDS; do
    run "$OUT/over-budget/refill5-s$seed" rgg 50 "$seed" --bin "$BIN" --refill 5 --byz-rate 10 --replay-copies 8
    run "$OUT/over-budget/refill20-s$seed" rgg 50 "$seed" --bin "$BIN" --byz-rate 40 --replay-copies 8
    if [ -n "$OLD" ]; then
        run "$OUT/over-budget/old-refill5-s$seed" rgg 50 "$seed" --bin "$OLD" --byz-rate 10 --replay-copies 8
    fi
    run "$OUT/flood/flood40-s$seed" rgg 50 "$seed" --bin "$BIN" --byz-rate 40 --replay-copies 1
    for kind in rgg mesh; do
        for n in 20 50 100; do
            run "$OUT/eventual/l20-$kind-$n-s$seed" "$kind" "$n" "$seed" --bin "$BIN" \
                --delay-ms 40 --jitter-ms 15 --loss 20 --drain 60
        done
    done
done
for d in over-budget flood eventual; do
    python3 "$HERE/report.py" "$OUT/$d"
done

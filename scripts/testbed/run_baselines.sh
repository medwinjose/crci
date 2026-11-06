#!/usr/bin/env bash
# Baselines on real processes: the three defence modes against attacks of growing intensity.
#
#   sudo scripts/testbed/run_baselines.sh OUT_DIR [crci-node binary] ["seeds"]
#
# Linux only; run as root. Builds nothing. rgg N = 50, 10 ms per hop, every run once per seed.
# Modes (`crci-node --defence`): full (the protocol as shipped), static-bucket (a per-sender token
# bucket and nothing else), vanilla (no admission limit, no bans). Attacks, by B from 10 s before
# its fault at 20 s:
#   none         B stays honest (no fault)
#   replay-N     B originates 40 messages/s and sends every frame N times (N = 2, 8, 32)
#   flood-R      B originates R fresh valid messages/s and never repeats a frame (R = 40, 100, 200)
# Existing runs (a summary.json in the run directory) are skipped.
set -euo pipefail

OUT=${1:?usage: run_baselines.sh OUT_DIR [crci-node binary] [seeds]}
BIN=${2:-target/release/crci-node}
SEEDS=${3:-"1 2 3"}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"

run() {
    local name=$1 seed=$2
    shift 2
    local dir="$OUT/$name-s$seed"
    if [ -f "$dir/summary.json" ]; then echo "skip $dir"; return; fi
    echo "=== $name (rgg, N=50, seed $seed) ==="
    python3 "$HERE/topology.py" rgg 50 "$seed" "$dir.topo.json" | tee "$dir.topology.txt"
    python3 "$HERE/testbed.py" --topo "$dir.topo.json" --out "$dir" --bin "$BIN" "$@"
    python3 "$HERE/analyze.py" "$dir" > /dev/null
}

for seed in $SEEDS; do
    for mode in full static-bucket vanilla; do
        run "$mode-none" "$seed" --defence "$mode" --no-fault
        for copies in 2 8 32; do
            run "$mode-replay-$copies" "$seed" --defence "$mode" --byz-rate 40 --replay-copies "$copies"
        done
        for rate in 40 100 200; do
            run "$mode-flood-$rate" "$seed" --defence "$mode" --byz-rate "$rate" --replay-copies 1
        done
    done
done
python3 "$HERE/report.py" "$OUT"
python3 "$HERE/baselines_table.py" "$OUT"

#!/usr/bin/env bash
# Invalid-signature flood: the Byzantine node B originates fresh messages from 10 s before the
# fault and, from the fault on, corrupts the signature of everything it sends, each frame
# sent COPIES times. Its neighbours must stop verifying after the third strike.
#
#   sudo scripts/testbed/run_flood.sh OUT_DIR [crci-node binary] ["seeds"]
#
# Linux only; run as root. Builds nothing. rgg N = 50, 40 messages/s from B (so 40, 320 and
# 2 560 forged frames/s on each of B's links for copies 1, 8, 64), a reference run with
# no fault (B stays silent) and a load-matched reference (B sends the same 40 messages/s with
# valid signatures, each once). Existing runs (summary.json) are skipped.
set -euo pipefail

OUT=${1:?usage: run_flood.sh OUT_DIR [crci-node binary] [seeds]}
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
    python3 "$HERE/testbed.py" --topo "$dir.topo.json" --out "$dir" --bin "$BIN" --byz-rate 40 "$@"
    python3 "$HERE/analyze.py" "$dir" > /dev/null
}

for seed in $SEEDS; do
    run reference "$seed" --no-fault
    run valid-x1 "$seed" --replay-copies 1
    # Not a forgery, so never struck: B floods fresh valid messages at 1 000/s (the fastest
    # origination rate of `--emit-interval-ms`, 1 ms); measured, with no pass/fail criterion.
    run fresh-1000 "$seed" --byz-rate 1000 --replay-copies 1
    for copies in 1 8 64; do
        run "forged-x$copies" "$seed" --replay-copies "$copies" --corrupt-signatures
    done
done
# Readmission: B forges for 10 s from the fault, then signs correctly (1 valid message/s). A ban
# lasts at least 300 s, so the window is 480 s. Its neighbours must readmit B through the probe.
run readmit 1 --window 480 --byz-rate 1 --replay-copies 1 --corrupt-signatures --corrupt-for 10
python3 "$HERE/report.py" "$OUT"
python3 "$HERE/flood_table.py" "$OUT"

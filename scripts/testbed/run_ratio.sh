#!/usr/bin/env bash
# Byzantine-ratio sweep: a share f of the nodes replays from 20 s on (every message sent twice;
# the attackers do not collude and do not originate), while honest traffic runs at 1 message/s.
#
#   sudo scripts/testbed/run_ratio.sh OUT_DIR [crci-node binary] ["seeds"] ["sizes"]
#
# Linux only; run as root. Builds nothing. One-way delay 40 ms +- 15 ms on every egress, window
# 60 s, 15 s of drain. f = 0 (no fault, the reference), 10, 20, 30 and 45 % of the nodes; each
# attacker set is drawn from the seed (never the source) exactly as scripts/sim/run_sim.py draws it,
# so the simulation's scale group can be compared with these runs node for node. Runs on rgg and
# mesh at every size and seed. Existing runs (a summary.json in the run directory) are skipped.
set -euo pipefail

OUT=${1:?usage: run_ratio.sh OUT_DIR [crci-node binary] [seeds] [sizes]}
BIN=${2:-target/release/crci-node}
SEEDS=${3:-"1 2 3"}
SIZES=${4:-"20 50 100"}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
COMMON="--delay-ms 40 --jitter-ms 15 --window 60 --drain 15"

run() {
    local f=$1 kind=$2 n=$3 seed=$4
    local topo="$OUT/topo-$kind-$n-s$seed.json"
    local dir="$OUT/f$f-$kind-$n-s$seed"
    if [ -f "$dir/summary.json" ]; then echo "skip $dir"; return; fi
    [ -f "$topo" ] || python3 "$HERE/topology.py" "$kind" "$n" "$seed" "$topo" > "$OUT/topo-$kind-$n-s$seed.txt"
    echo "=== f=$f% $kind N=$n seed=$seed ==="
    local extra="--no-fault"
    [ "$f" != "0" ] && extra="--attackers-pct $f"
    # shellcheck disable=SC2086
    python3 "$HERE/testbed.py" --topo "$topo" --out "$dir" --bin "$BIN" $COMMON $extra
    python3 "$HERE/analyze.py" "$dir" > /dev/null
}

for seed in $SEEDS; do
    for kind in rgg mesh; do
        for n in $SIZES; do
            for f in 0 10 20 30 45; do
                run "$f" "$kind" "$n" "$seed"
            done
        done
    done
done
python3 "$HERE/report.py" "$OUT" > /dev/null
python3 "$HERE/ratio_table.py" "$OUT"

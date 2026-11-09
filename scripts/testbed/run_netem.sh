#!/usr/bin/env bash
# Run the netem impairment experiments (one-way delay 40 ms +- 15 ms jitter on every node's egress)
# and analyse each run. Existing runs (a summary.json in the run directory) are skipped, so an
# interrupted sweep can be resumed with the same arguments.
#
#   sudo scripts/testbed/run_netem.sh OUT_DIR [crci-node binary] ["seeds"] ["sizes"]
#
# Linux only; run as root. Conditions (name: loss % / reorder %):
#   l0 l5 l10 l15 l20   loss 0/5/10/15/20 %, no reordering
#   r5 r10 r15          reordering 5/10/15 %, no loss
#   l10r10              loss 10 % and reordering 10 %
# Every condition runs on rgg and mesh at every size and every seed (the seed selects the topology
# instance, the roles and the traffic schedule; the kernel's netem randomness is not seeded).
# Traffic is 1 message/s in total, as in run_all.sh; the relay B repeats every message from 20 s on.
set -euo pipefail

OUT=${1:?usage: run_netem.sh OUT_DIR [crci-node binary] [seeds] [sizes]}
BIN=${2:-target/release/crci-node}
SEEDS=${3:-"1 2 3"}
SIZES=${4:-"20 50 100"}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"

CONDITIONS="l0:0:0 l5:5:0 l10:10:0 l15:15:0 l20:20:0 r5:0:5 r10:0:10 r15:0:15 l10r10:10:10"

for seed in $SEEDS; do
  for kind in rgg mesh; do
    for n in $SIZES; do
      topo="$OUT/topo-$kind-$n-s$seed.json"
      [ -f "$topo" ] || python3 "$HERE/topology.py" "$kind" "$n" "$seed" "$topo" > "$OUT/topo-$kind-$n-s$seed.txt"
      for cond in $CONDITIONS; do
        IFS=: read -r name loss reorder <<< "$cond"
        run="$OUT/netem-$name-$kind-$n-s$seed"
        if [ -f "$run/summary.json" ]; then echo "skip $run"; continue; fi
        echo "=== $name $kind N=$n seed=$seed (loss $loss %, reorder $reorder %) ==="
        python3 "$HERE/testbed.py" --topo "$topo" --out "$run" --bin "$BIN" \
            --delay-ms 40 --jitter-ms 15 --loss "$loss" --reorder "$reorder"
        python3 "$HERE/analyze.py" "$run" > /dev/null
      done
    done
  done
done
python3 "$HERE/report.py" "$OUT" > /dev/null
python3 "$HERE/netem_table.py" "$OUT"

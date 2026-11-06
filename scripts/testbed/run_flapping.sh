#!/usr/bin/env bash
# Link flapping: a share of the overlay links goes down and up again every 5-10 s while honest
# traffic runs, and the nodes must neither ban nor strike each other.
#
#   sudo scripts/testbed/run_flapping.sh OUT_DIR [crci-node binary] ["seeds"] ["sizes"]
#
# Linux only; run as root. Builds nothing. One-way delay 40 ms +- 15 ms on every egress, 1 message/s
# in total. Window 90 s from T0; links flap from 10 s to 70 s and are all up again for the last
# 20 s; the nodes run 60 s after the window so late recoveries count. Existing runs (a
# summary.json in the run directory) are skipped, so an interrupted sweep resumes.
#
# Conditions of the honest runs (no fault; every ban or strike is then a false one):
#   ref      no flapping
#   sym20    20 % of the links, both directions down together
#   asym20   20 % of the links, one direction down (chosen per link)
#   sym50    50 % of the links, both directions (heavy: the mesh partitions for stretches)
#   asym50   50 % of the links, one direction
# Conditions with the replaying relay B (fault at 20 s, every frame sent twice), N = 50 only:
#   atk-ref    no flapping
#   atk-sym20  20 % of the links flap symmetrically (B's links included)
# Every condition runs on rgg and mesh at every size and seed (the seed selects the topology,
# the roles, the traffic schedule and the set of flapping links).
set -euo pipefail

OUT=${1:?usage: run_flapping.sh OUT_DIR [crci-node binary] [seeds] [sizes]}
BIN=${2:-target/release/crci-node}
SEEDS=${3:-"1 2 3"}
SIZES=${4:-"20 50 100"}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"

HONEST="ref:0:symmetric sym20:0.2:symmetric asym20:0.2:asymmetric sym50:0.5:symmetric asym50:0.5:asymmetric"
ATTACK="atk-ref:0:symmetric atk-sym20:0.2:symmetric"
COMMON="--delay-ms 40 --jitter-ms 15 --window 90 --drain 60"

run() {
    local name=$1 kind=$2 n=$3 seed=$4 fraction=$5 mode=$6 fault=$7
    local topo="$OUT/topo-$kind-$n-s$seed.json"
    local dir="$OUT/$name-$kind-$n-s$seed"
    if [ -f "$dir/summary.json" ]; then echo "skip $dir"; return; fi
    [ -f "$topo" ] || python3 "$HERE/topology.py" "$kind" "$n" "$seed" "$topo" > "$OUT/topo-$kind-$n-s$seed.txt"
    echo "=== $name $kind N=$n seed=$seed (flap $fraction $mode, fault: $fault) ==="
    local extra=""
    [ "$fault" = "none" ] && extra="--no-fault"
    if [ "$fraction" != "0" ]; then extra="$extra --flap-fraction $fraction --flap-mode $mode"; fi
    # shellcheck disable=SC2086
    python3 "$HERE/testbed.py" --topo "$topo" --out "$dir" --bin "$BIN" $COMMON $extra
    python3 "$HERE/analyze.py" "$dir" > /dev/null
}

for seed in $SEEDS; do
    for kind in rgg mesh; do
        for n in $SIZES; do
            for cond in $HONEST; do
                IFS=: read -r name fraction mode <<< "$cond"
                run "$name" "$kind" "$n" "$seed" "$fraction" "$mode" none
            done
        done
        # Attack runs only at N = 50 (when it is among the sizes).
        case " $SIZES " in *" 50 "*)
            for cond in $ATTACK; do
                IFS=: read -r name fraction mode <<< "$cond"
                run "$name" "$kind" 50 "$seed" "$fraction" "$mode" replay
            done ;;
        esac
    done
done
python3 "$HERE/report.py" "$OUT" > /dev/null
python3 "$HERE/flap_table.py" "$OUT"

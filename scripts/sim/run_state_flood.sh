#!/bin/bash
# Memory of one node under floods of validly signed messages (examples/state_flood): one million
# distinct origins with and without the application pipeline, one million messages over 100
# origins, and one million distinct origins and senders. About 8 minutes per run, four at a time.
#
#   scripts/sim/run_state_flood.sh OUT_DIR [state_flood binary]
#
# Writes one CSV per run, heap.svg and heap.txt; fails if a run's heap is not flat.
set -eu
OUT=${1:?usage: run_state_flood.sh OUT_DIR [state_flood binary]}
BIN=${2:-target/release/examples/state_flood}
mkdir -p "$OUT"
sha256sum "$BIN" > "$OUT/binary.sha256"

# The node's own log goes to stdout; the program's CSV goes to stderr.
run() {
    name=$1
    shift
    "$BIN" "$@" > /dev/null 2> "$OUT/$name.csv"
}
run distinct-origins --messages 1000000 --origins 1000000 --senders 50 --every 50000 &
run distinct-origins-protocol-only --messages 1000000 --origins 1000000 --senders 50 --every 50000 --no-pipeline &
run few-origins --messages 1000000 --origins 100 --senders 50 --every 50000 &
run distinct-senders --messages 1000000 --origins 1000000 --senders 1000000 --every 50000 &
wait

python3 scripts/sim/state_flood_plot.py "$OUT/heap.svg" "Live heap under a flood of signed messages" \
    "one million origins=$OUT/distinct-origins.csv" \
    "one million origins, protocol state only=$OUT/distinct-origins-protocol-only.csv" \
    "100 origins=$OUT/few-origins.csv" \
    "one million origins and senders=$OUT/distinct-senders.csv" | tee "$OUT/heap.txt"

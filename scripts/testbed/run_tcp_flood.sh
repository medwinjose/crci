#!/bin/bash
# A flood of validly signed messages through one real crci-node, from many identities.
#
#   sudo scripts/testbed/run_tcp_flood.sh OUT_DIR [crci-node] [tcp_flood] [messages] [connections]
#
# In a fresh network namespace on loopback: a receiver (no peers), one honest node dialling it
# and emitting 1 message/s, and `examples/tcp_flood` sending `messages` (default 10^6) over
# `connections` (default 90) authenticated connections at 20 messages/s each, every message from
# a new origin identity. Samples the receiver every 5 s into samples.csv (RSS, CPU seconds, log
# bytes) and writes summary.txt. Linux, root.
set -eu
OUT=${1:?usage: run_tcp_flood.sh OUT_DIR [crci-node] [tcp_flood] [messages] [connections]}
BIN=${2:-target/release/crci-node}
FLOOD=${3:-target/release/examples/tcp_flood}
MESSAGES=${4:-1000000}
CONNECTIONS=${5:-90}
NS=crci-tcpflood
mkdir -p "$OUT"
sha256sum "$BIN" "$FLOOD" > "$OUT/binary.sha256"

ip netns del $NS 2>/dev/null || true
ip netns add $NS
ip netns exec $NS ip link set lo up
cleanup() { ip netns pids $NS 2>/dev/null | xargs -r kill 2>/dev/null || true; sleep 1; ip netns del $NS 2>/dev/null || true; }
trap cleanup EXIT

RATE=20
DURATION=$(( MESSAGES / (CONNECTIONS * RATE) + 60 ))
ip netns exec $NS "$BIN" --listen 127.0.0.1:7001 --label receiver > "$OUT/receiver.log" 2>&1 &
RECEIVER=$!
sleep 2
ip netns exec $NS "$BIN" --listen 127.0.0.1:7002 --label honest --dial 127.0.0.1:7001 \
    --emit-count "$DURATION" --emit-interval-ms 1000 --emit-delay-ms 3000 > "$OUT/honest.log" 2>&1 &
sleep 5

echo "t_s,rss_kib,cpu_s,log_bytes" > "$OUT/samples.csv"
TICKS=$(getconf CLK_TCK)
(
    start=$(date +%s)
    while kill -0 $RECEIVER 2>/dev/null; do
        rss=$(awk '/VmRSS/{print $2}' /proc/$RECEIVER/status 2>/dev/null || echo 0)
        cpu=$(awk -v t="$TICKS" '{printf "%.2f", ($14 + $15) / t}' /proc/$RECEIVER/stat 2>/dev/null || echo 0)
        bytes=$(stat -c %s "$OUT/receiver.log")
        echo "$(( $(date +%s) - start )),$rss,$cpu,$bytes" >> "$OUT/samples.csv"
        sleep 5
    done
) &
SAMPLER=$!

ip netns exec $NS "$FLOOD" --target 127.0.0.1:7001 --connections "$CONNECTIONS" --rate $RATE \
    --messages "$MESSAGES" > "$OUT/flood.txt" 2>&1
sleep 10
kill $SAMPLER 2>/dev/null || true
cleanup
trap - EXIT

{
    cat "$OUT/flood.txt"
    # Acceptance lines held back by the node's log limit are counted by the line printed after them.
    grep "accepted '" "$OUT/receiver.log" | awk '{ n++; if (match($0, /\(\+[0-9]+ not printed\)/))
        h += substr($0, RSTART + 2, RLENGTH - 15) }
        END { printf "receiver acceptances: %d (lines printed %d, held back %d)\n", n + h, n, h }'
    # The honest node's key-derived ID, from its start-up line; the receiver logs peers by ID.
    honest_id=$(grep -m1 -o "^Node honest id [0-9a-f]*" "$OUT/honest.log" | awk '{print $4}')
    originated=$(grep -o "originated '[^']*'" "$OUT/honest.log" | sed "s/originated '//; s/'//" | sort -u)
    echo "honest originated: $(echo "$originated" | grep -c . || true)"
    echo "honest acceptance lines printed by the receiver: $(grep -o "accepted '[^']*' (origin $honest_id," "$OUT/receiver.log" | sort -u | wc -l)"
    echo "honest rate-limited lines: $(grep -c "peer $honest_id exceeded rate limit" "$OUT/receiver.log" || true)"
    echo "honest banned: $(grep -c "banned $honest_id " "$OUT/receiver.log" || true)"
    echo "receiver log bytes: $(stat -c %s "$OUT/receiver.log")"
    awk -F, 'NR > 1 { n++; rss[n] = $2; cpu = $3; t = $1 } END {
        printf "samples=%d run_s=%d rss_at_tenth_kib=%d rss_end_kib=%d rss_max_kib=%d cpu_s=%s\n",
            n, t, rss[int(n / 10) + 1], rss[n], max_of(rss, n), cpu }
        function max_of(a, k,   i, m) { m = 0; for (i = 1; i <= k; i++) if (a[i] > m) m = a[i]; return m }' \
        "$OUT/samples.csv"
} | tee "$OUT/summary.txt"

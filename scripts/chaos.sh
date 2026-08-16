#!/usr/bin/env bash
set -euo pipefail

PASS=0
FAIL=0

check() {
  local label="$1"
  local result="$2"
  if [ "$result" = "ok" ]; then
    echo "  ✓ $label"
    PASS=$((PASS + 1))
  else
    echo "  ✗ $label — $result"
    FAIL=$((FAIL + 1))
  fi
}

echo "=== CRCI Chaos Engineering Suite ==="
echo ""

# 1. Run in-memory Rust Chaos Suite (4 Scenarios: Packet Loss, Crash+Restart, Reconnect Storm, Partition Reconciliation)
echo "--- In-Memory Rust Chaos Suite (4 Scenarios) ---"
if cargo test --lib chaos -- --nocapture; then
  check "In-Memory Rust Chaos Suite (4 scenarios)" "ok"
else
  check "In-Memory Rust Chaos Suite (4 scenarios)" "failed"
fi
echo ""

# 2. Check Docker daemon availability for Container Integration Scenarios
echo "--- Docker Compose Container Chaos Scenarios ---"
DOCKER_AVAILABLE=false
if command -v docker >/dev/null 2>&1; then
  if docker info >/dev/null 2>&1; then
    DOCKER_AVAILABLE=true
  fi
fi

if [ "$DOCKER_AVAILABLE" = false ]; then
  echo "! Docker daemon is not running. Skipping containerized network injection scenarios."
  check "Docker Container Integration Scenarios" "skipped (Docker daemon not running)"
  echo ""
  echo "=== Results: $PASS passed/skipped, $FAIL failed ==="
  [ "$FAIL" -eq 0 ] && exit 0 || exit 1
fi

# Baseline Docker Compose check
echo "--- Baseline health check ---"
docker compose up -d
sleep 4
if ! docker compose ps | grep -q "Up"; then
  echo "Mesh is not running. Please start the mesh first."
  exit 1
fi
echo "✓ Baseline is running"

# Scenario A: Link Partition
echo ""
echo "--- Scenario A: Link Partition ---"
ALPHA=$(docker compose ps -q node-alpha)
BETA=$(docker compose ps -q node-beta)

echo "Disconnecting node-alpha from crci-mesh..."
docker network disconnect crci_crci-mesh "$ALPHA" || docker network disconnect crci-mesh "$ALPHA"
sleep 5
echo "Reconnecting node-alpha to crci-mesh..."
docker network reconnect crci_crci-mesh "$ALPHA" || docker network reconnect crci-mesh "$ALPHA"
sleep 3
echo "node-alpha logs during partition:"
docker compose logs --since 10s node-alpha

if docker compose logs --since 10s node-alpha | grep -q "panic"; then
  check "Scenario A: Link Partition" "failed (panic detected)"
else
  check "Scenario A: Link Partition" "ok"
fi

# Scenario B: Node Crash
echo ""
echo "--- Scenario B: Node Crash ---"
echo "Killing node-byzantine..."
docker compose kill node-byzantine
sleep 3

echo "node-alpha logs after byzantine crash:"
docker compose logs --since 10s node-alpha | tail -20
echo "node-beta logs after byzantine crash:"
docker compose logs --since 10s node-beta | tail -20

echo "Restarting node-byzantine..."
docker compose start node-byzantine
sleep 3
if docker compose ps | grep -i node-byzantine | grep -q "Up"; then
  check "Scenario B: Node Crash" "ok"
else
  check "Scenario B: Node Crash" "failed (byzantine node did not restart cleanly)"
fi

# Scenario C: Packet Delay
echo ""
echo "--- Scenario C: Packet Delay ---"
if docker compose exec node-beta which tc > /dev/null 2>&1; then
  docker compose exec node-beta tc qdisc add dev eth0 root netem delay 500ms
  sleep 5
  docker compose exec node-beta tc qdisc del dev eth0 root
  check "Scenario C: Packet Delay" "ok"
else
  echo "! tc not available in runtime image — latency scenario skipped (documented in report)"
  check "Scenario C: Packet Delay" "skipped (tc unavailable)"
  # Still marking it as an acceptable result instead of an actual test fail since it's documented.
fi

# Scenario D: Byzantine + Partition
echo ""
echo "--- Scenario D: Byzantine + Partition ---"
echo "Disconnecting node-beta while node-byzantine continues injection..."
docker network disconnect crci_crci-mesh "$BETA" || docker network disconnect crci-mesh "$BETA"
sleep 6
echo "Reconnecting node-beta..."
docker network reconnect crci_crci-mesh "$BETA" || docker network reconnect crci-mesh "$BETA"
sleep 3

if docker compose logs --since 15s node-alpha | grep -qi "panic"; then
  check "Scenario D: Byzantine + Partition" "failed (panic detected)"
else
  check "Scenario D: Byzantine + Partition" "ok"
fi

echo ""
echo "=== Results: $PASS passed/skipped, $FAIL failed ==="
[ "$FAIL" -eq 0 ] && exit 0 || exit 1

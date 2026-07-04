#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "       CRCI Demo Stack CI Validation      "
echo "=========================================="
echo ""

# 1. Validate Docker Compose config syntax
echo "Checking docker-compose.yml syntax..."
if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  docker compose config > /dev/null
  echo "✓ docker-compose.yml is valid."
else
  echo "docker / docker compose not found. Skipping config validation."
fi

# 2. Validate bash script syntax
echo "Checking scripts/demo.sh syntax..."
bash -n scripts/demo.sh
echo "✓ scripts/demo.sh syntax is valid."

echo ""
echo "Demo orchestration stack validation successful!"

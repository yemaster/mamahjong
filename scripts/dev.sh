#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname -- "$SCRIPT_DIR")"
PROJECT_NAME="${MAMAHJONG_DEV_PROJECT:-mamahjong}"

cd "$PROJECT_ROOT"

docker compose -p "$PROJECT_NAME" up \
  --detach \
  --build \
  --remove-orphans \
  --wait \
  --wait-timeout "${MAMAHJONG_DEV_WAIT_TIMEOUT:-120}"

docker compose -p "$PROJECT_NAME" ps

echo
echo "MaMahjong development environment is ready: http://127.0.0.1:${MAMAHJONG_WEB_PORT:-8080}/game/"

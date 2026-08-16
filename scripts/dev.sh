#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname -- "$SCRIPT_DIR")"
PROJECT_NAME="${MAMAHJONG_DEV_PROJECT:-mamahjong}"
ADMIN_PASSWORD_IS_DEFAULT=false

# 本地开发才开：游戏客户端进入开发模式，可以用键盘 q..p,a,s,d 逐张自定义自己的
# 手牌。这个开关经 compose 的 build-arg 打进 web 镜像，别处（生产、其它脚本）不设。
export MAMAHJONG_DEV_MODE="${MAMAHJONG_DEV_MODE:-true}"

if [[ -z "${MAMAHJONG_ADMIN_PASSWORD:-}" ]]; then
  export MAMAHJONG_ADMIN_PASSWORD="abc123456"
  export MAMAHJONG_ADMIN_ALLOW_INSECURE_PASSWORD="true"
  ADMIN_PASSWORD_IS_DEFAULT=true
fi

cd "$PROJECT_ROOT"

docker compose -p "$PROJECT_NAME" --profile admin up \
  --detach \
  --build \
  --remove-orphans \
  --wait \
  --wait-timeout "${MAMAHJONG_DEV_WAIT_TIMEOUT:-120}"

docker compose -p "$PROJECT_NAME" --profile admin ps

echo
echo "MaMahjong game web:  http://127.0.0.1:${MAMAHJONG_WEB_PORT:-8080}/game/"
echo "MaMahjong admin web: http://127.0.0.1:${MAMAHJONG_WEB_PORT:-8080}/admin/"
if [[ "$ADMIN_PASSWORD_IS_DEFAULT" == "true" ]]; then
  echo "Admin login: ${MAMAHJONG_ADMIN_LOGIN_NAME:-admin} / ${MAMAHJONG_ADMIN_PASSWORD}"
fi

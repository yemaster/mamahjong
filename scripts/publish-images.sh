#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  echo "Usage: $0 <version>"
  echo "Example: $0 0.1.1"
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 2
fi

VERSION="$1"
if [[ ! "$VERSION" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]]; then
  echo "Invalid image version: $VERSION" >&2
  exit 2
fi

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname -- "$SCRIPT_DIR")"
REGISTRY="${MAMAHJONG_REGISTRY:-registry.abstrax.cn/mamahjong}"
PLATFORMS="${MAMAHJONG_PLATFORMS:-linux/amd64,linux/arm64}"
PUBLISH_LATEST="${MAMAHJONG_PUBLISH_LATEST:-true}"
BUILD_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

cd "$PROJECT_ROOT"

VCS_REF="$(git rev-parse HEAD)"
if [[ -n "$(git status --porcelain)" ]]; then
  VCS_REF="${VCS_REF}-dirty"
  echo "Warning: publishing from a dirty working tree." >&2
fi

for TARGET in server web; do
  TAG_ARGS=(--tag "${REGISTRY}/${TARGET}:${VERSION}")
  if [[ "$PUBLISH_LATEST" == "true" ]]; then
    TAG_ARGS+=(--tag "${REGISTRY}/${TARGET}:latest")
  fi

  echo "Building and pushing ${REGISTRY}/${TARGET}:${VERSION} for ${PLATFORMS}"
  docker buildx build \
    --target "$TARGET" \
    --platform "$PLATFORMS" \
    --push \
    "${TAG_ARGS[@]}" \
    --build-arg "VERSION=${VERSION}" \
    --build-arg "VCS_REF=${VCS_REF}" \
    --build-arg "BUILD_DATE=${BUILD_DATE}" \
    "$PROJECT_ROOT"
done

echo "Published server and web images with tag ${VERSION}."
echo "Production can now be updated with:"
echo "  docker compose --env-file .env.production pull"
echo "  docker compose --env-file .env.production up --detach"

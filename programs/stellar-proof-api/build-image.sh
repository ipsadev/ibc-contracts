#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
config="${CONFIG:-$root/programs/stellar-proof-api/testnet.json}"
image="${IMAGE:-amandagonsalvesx/ipsa-proof-api}"
tag="${TAG:-$(git -C "$root" rev-parse --short=12 HEAD)}"
platform="${PLATFORM:-linux/arm64}"
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

cd "$root"

jq -r '.. | strings | select(test("/release/|^ibc-solidity/out/"))' "$config" | sort -u | while read -r path; do
  [ -f "$path" ] || { echo "missing $path: build it before the image" >&2; exit 1; }
  mkdir -p "$staging/$(dirname "$path")"
  cp "$path" "$staging/$path"
  echo "staged $path ($(shasum -a 256 "$path" | cut -c1-16))"
done

docker buildx build \
  --platform "$platform" \
  --build-context artifacts="$staging" \
  --build-arg VCS_REF="$tag" \
  -f programs/stellar-proof-api/Dockerfile \
  -t "$image:$tag" \
  "${@:---load}" \
  .

echo "built $image:$tag ($platform)"

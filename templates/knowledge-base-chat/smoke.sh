#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

run_check() {
  local workdir="$1"
  local command="$2"

  echo "==> ${workdir}: ${command}"
  (cd "${ROOT_DIR}/${workdir}" && bash -lc "${command}")
}

run_check "apps/chat-web" "pnpm test"
run_check "backend" "cargo test -p backend knowledge --offline"

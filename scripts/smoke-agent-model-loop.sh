#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:4398}"
TOKEN="${TOKEN:-}"

if [[ -z "${TOKEN}" ]]; then
  echo "TOKEN is required" >&2
  exit 2
fi

curl -fsS \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"input":"search the training handbook for customer data policy","runtimeMode":"model_loop","autoApprove":false,"budget":{"maxSteps":8,"maxToolCalls":1,"maxSeconds":60,"maxCostCents":0}}' \
  "${BASE_URL}/ai/agents/runs"

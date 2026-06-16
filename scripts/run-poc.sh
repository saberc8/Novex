#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${ROOT_DIR}/infra/docker-compose.yml"
POC_ENV_FILE="${ROOT_DIR}/infra/.env.poc"
POC_ENV_EXAMPLE="${ROOT_DIR}/infra/.env.poc.example"
COMMAND="${1:-up}"

COMPOSE=()
POC_SERVICES=(
  backend
  parser-worker
  admin
  training-web
  chat-web
  agent-workspace
)
COMMON_REQUIRED_CONTAINERS=(
  docker-postgres
  docker-redis
  docker-rabbitmq
  docker-minio
  docker-milvus
  docker-etcd
  docker-neo4j
)

main() {
  cd "${ROOT_DIR}"
  if [[ "${COMMAND}" != "help" && "${COMMAND}" != "-h" && "${COMMAND}" != "--help" ]]; then
    ensure_poc_env_file
    load_poc_env
    build_compose_command
  fi

  case "${COMMAND}" in
    up)
      require_docker
      ensure_parser_callback_token
      check_live_ai_env
      require_common_docker_services
      ensure_common_postgres_database
      require_local_images
      print_urls
      exec "${COMPOSE[@]}" up --pull never "${POC_SERVICES[@]}"
      ;;
    down)
      require_docker
      exec "${COMPOSE[@]}" down
      ;;
    logs)
      require_docker
      exec "${COMPOSE[@]}" logs -f "${POC_SERVICES[@]}"
      ;;
    status|ps)
      require_docker
      exec "${COMPOSE[@]}" ps
      ;;
    env|check-env)
      ensure_parser_callback_token
      check_live_ai_env
      ;;
    pull)
      require_docker
      pull_missing_images
      ;;
    help|-h|--help)
      usage
      ;;
    *)
      echo "Unknown command: ${COMMAND}" >&2
      usage >&2
      exit 2
      ;;
  esac
}

ensure_poc_env_file() {
  if [[ -f "${POC_ENV_FILE}" ]]; then
    return
  fi

  if [[ ! -f "${POC_ENV_EXAMPLE}" ]]; then
    echo "Missing env template: infra/.env.poc.example" >&2
    exit 1
  fi

  cp "${POC_ENV_EXAMPLE}" "${POC_ENV_FILE}"
  echo "Created env: infra/.env.poc from infra/.env.poc.example"
}

load_poc_env() {
  set -a
  # shellcheck disable=SC1090
  . "${POC_ENV_FILE}"
  set +a
  echo "Loaded env: infra/.env.poc"
}

build_compose_command() {
  COMPOSE=(
    docker compose
    --env-file "${POC_ENV_FILE}"
    -f "${COMPOSE_FILE}"
    --profile parser
    --profile apps
  )
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "Docker is required but was not found in PATH." >&2
    exit 1
  fi
  if ! docker compose version >/dev/null 2>&1; then
    echo "Docker Compose v2 is required: docker compose version failed." >&2
    exit 1
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "Docker daemon is not running or not reachable." >&2
    exit 1
  fi
}

require_local_images() {
  local missing=()
  local image

  while IFS= read -r image; do
    missing+=("${image}")
  done < <(missing_local_images)

  if [[ "${#missing[@]}" -eq 0 ]]; then
    return
  fi

  echo "Missing local Docker images; the POC script will not auto-pull during up:" >&2
  printf '  %s\n' "${missing[@]}" >&2
  print_local_image_alternatives "${missing[@]}" >&2
  cat >&2 <<'EOF'

Options:
  1. Run './scripts/run-poc.sh pull' when Docker Hub access is available.
  2. Edit infra/.env.poc to point RUST_IMAGE, NODE_IMAGE, or PYTHON_IMAGE at tags you already have locally.

EOF
  exit 1
}

require_common_docker_services() {
  local network="${COMMON_DOCKER_NETWORK:-docker-common_default}"
  local bad=()
  local container
  local status
  local health

  if ! docker network inspect "${network}" >/dev/null 2>&1; then
    echo "Missing shared Docker network: ${network}" >&2
    print_common_stack_hint >&2
    exit 1
  fi

  for container in "${COMMON_REQUIRED_CONTAINERS[@]}"; do
    if ! docker inspect "${container}" >/dev/null 2>&1; then
      bad+=("${container}: missing")
      continue
    fi

    status="$(docker inspect --format '{{.State.Status}}' "${container}")"
    health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "${container}")"
    if [[ "${status}" != "running" ]]; then
      bad+=("${container}: ${status}")
      continue
    fi
    if [[ "${health}" != "none" && "${health}" != "healthy" ]]; then
      bad+=("${container}: ${health}")
    fi
  done

  if [[ "${#bad[@]}" -eq 0 ]]; then
    return
  fi

  echo "Shared docker-common services are not ready:" >&2
  printf '  %s\n' "${bad[@]}" >&2
  print_common_stack_hint >&2
  exit 1
}

ensure_common_postgres_database() {
  local container="${COMMON_POSTGRES_CONTAINER:-docker-postgres}"
  local user="${COMMON_POSTGRES_USER:-postgres}"
  local password="${COMMON_POSTGRES_PASSWORD:-postgres}"
  local database="${COMMON_POSTGRES_DATABASE:-novex}"
  local exists

  if [[ ! "${database}" =~ ^[A-Za-z0-9_]+$ ]]; then
    echo "COMMON_POSTGRES_DATABASE must contain only letters, numbers, and underscores: ${database}" >&2
    exit 1
  fi

  exists="$(
    docker exec -e PGPASSWORD="${password}" "${container}" \
      psql -U "${user}" -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = '${database}'" \
      | tr -d '[:space:]'
  )"
  if [[ "${exists}" == "1" ]]; then
    return
  fi

  echo "Creating shared Postgres database: ${database}"
  docker exec -e PGPASSWORD="${password}" "${container}" createdb -U "${user}" "${database}"
}

print_common_stack_hint() {
  cat <<'EOF'
Start or repair the shared stack first:
  cd /Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom
  docker compose up -d postgres redis rabbitmq etcd minio milvus attu neo4j
EOF
}

pull_missing_images() {
  local missing=()
  local failed=()
  local image

  while IFS= read -r image; do
    missing+=("${image}")
  done < <(missing_local_images)

  if [[ "${#missing[@]}" -eq 0 ]]; then
    echo "All compose images already exist locally; nothing to pull."
    return
  fi

  echo "Pulling missing Docker images only:"
  printf '  %s\n' "${missing[@]}"
  for image in "${missing[@]}"; do
    if ! docker pull "${image}"; then
      failed+=("${image}")
    fi
  done

  if [[ "${#failed[@]}" -eq 0 ]]; then
    return
  fi

  echo "Failed to pull these Docker images:" >&2
  printf '  %s\n' "${failed[@]}" >&2
  print_local_image_alternatives "${failed[@]}" >&2
  exit 1
}

missing_local_images() {
  local image

  while IFS= read -r image; do
    if [[ -z "${image}" ]]; then
      continue
    fi
    if ! docker image inspect "${image}" >/dev/null 2>&1; then
      echo "${image}"
    fi
  done < <("${COMPOSE[@]}" config --images | sort -u)
}

print_local_image_alternatives() {
  local image
  local repository
  local alternatives

  for image in "$@"; do
    repository="${image%:*}"
    alternatives="$(docker images --format '{{.Repository}}:{{.Tag}}' | grep -E "^${repository//\//\\/}:" | sort || true)"
    if [[ -n "${alternatives}" ]]; then
      echo "Local tags for ${repository}:"
      printf '  %s\n' ${alternatives}
    fi
  done
}

ensure_parser_callback_token() {
  if [[ -n "${PARSER_CALLBACK_TOKEN:-}" ]]; then
    export PARSER_CALLBACK_TOKEN
    return
  fi

  if command -v openssl >/dev/null 2>&1; then
    PARSER_CALLBACK_TOKEN="local-parser-$(openssl rand -hex 24)"
  else
    PARSER_CALLBACK_TOKEN="local-parser-callback-token-change-me"
  fi
  export PARSER_CALLBACK_TOKEN
}

check_live_ai_env() {
  echo
  echo "Environment check"
  echo "-----------------"
  check_group "LLM" "live chat and RAG answer generation" \
    LLM_API_KEY LLM_BASE_URL LLM_MODEL
  check_group "Embedding" "semantic chunk embedding and Milvus vector recall" \
    EMBEDDING_API_KEY EMBEDDING_BASE_URL EMBEDDING_MODEL
  check_group "Reranker" "RAG rerank scoring; local fallback is used when missing" \
    RERANKER_API_KEY RERANKER_BASE_URL RERANKER_MODEL
  check_group "Draw" "image generation tool route for agent media jobs" \
    RIGHT_CODE_DRAW_BASE_URL RIGHT_CODE_DRAW_API_KEY
  check_group "MinerU" "PDF, Office, and image document parsing" \
    MINERU_TOKEN PARSER_WORKER_MODE MINERU_TIMEOUT_SECONDS
  check_group "Parser callback" "parser-worker backend callback authentication" \
    PARSER_CALLBACK_TOKEN
  echo
}

check_group() {
  local name="$1"
  local impact="$2"
  shift 2

  local missing=()
  local present=()
  local var
  for var in "$@"; do
    if [[ -n "${!var:-}" ]]; then
      present+=("${var}=$(mask_value "${!var}")")
    else
      missing+=("${var}")
    fi
  done

  if [[ "${#missing[@]}" -eq 0 ]]; then
    echo "[OK]   ${name}: ${present[*]}"
  else
    echo "[WARN] ${name}: missing ${missing[*]}"
    echo "       Impact: ${impact}"
  fi
}

mask_value() {
  local value="$1"
  local length="${#value}"
  if [[ "${length}" -le 4 ]]; then
    echo "***"
  elif [[ "${length}" -le 12 ]]; then
    echo "${value:0:2}****${value: -2}"
  else
    echo "${value:0:4}****${value: -4}"
  fi
}

print_urls() {
  cat <<EOF
Starting Novex POC stack
------------------------
Backend:          http://localhost:${BACKEND_PORT:-4398}
Admin:            http://localhost:${ADMIN_PORT:-4399}
Training Web:     http://localhost:${TRAINING_WEB_PORT:-4401}
Chat Web:         http://localhost:${CHAT_WEB_PORT:-4402}
Agent Workspace:  http://localhost:${AGENT_WORKSPACE_PORT:-4403}
RabbitMQ UI:      ${RABBITMQ_MANAGEMENT_URL:-http://localhost:15673}
MinIO Console:    ${MINIO_CONSOLE_URL:-http://localhost:19011}
Attu:             ${ATTU_URL:-http://localhost:18000}
Neo4j Browser:    ${NEO4J_BROWSER_URL:-http://localhost:17474}

RabbitMQ default login: ${RABBITMQ_DEFAULT_USER:-guest} / ${RABBITMQ_DEFAULT_PASS:-guest}
EOF
}

usage() {
  cat <<EOF
Usage: scripts/run-poc.sh [command]

Commands:
  up         Check docker-common, then start backend, parser-worker, and POC frontends
  down       Stop and remove compose services
  logs       Follow logs for the POC stack
  status     Show compose service status
  env        Check live AI environment variables only
  pull       Pull compose images
  help       Show this help

Single local env entry:
  infra/.env.poc

Agent model-loop smoke:
  TOKEN="\$ADMIN_TOKEN" ./scripts/smoke-agent-model-loop.sh

Default URLs:
  Backend          http://localhost:4398
  Admin            http://localhost:4399
  Training Web     http://localhost:4401
  Chat Web         http://localhost:4402
  Agent Workspace  http://localhost:4403
  RabbitMQ UI      http://localhost:15673
  MinIO Console    http://localhost:19011
  Attu             http://localhost:18000
EOF
}

main "$@"

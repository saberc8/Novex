#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ROOT_DIR}/.env"
ENV_EXAMPLE="${ROOT_DIR}/.env.example"
COMMAND="${1:-check}"

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
    load_env
  fi

  case "${COMMAND}" in
    check|up)
      require_docker
      ensure_parser_callback_token
      check_live_ai_env
      require_common_docker_services
      ensure_common_postgres_database
      print_local_commands
      ;;
    env|check-env)
      ensure_parser_callback_token
      check_live_ai_env
      ;;
    commands)
      ensure_parser_callback_token
      print_local_commands
      ;;
    down)
      print_no_managed_processes
      ;;
    status|ps)
      print_local_status_hint
      ;;
    logs)
      print_local_logs_hint
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
  if [[ -f "${ENV_FILE}" ]]; then
    return
  fi

  if [[ ! -f "${ENV_EXAMPLE}" ]]; then
    echo "Missing env template: .env.example" >&2
    exit 1
  fi

  cp "${ENV_EXAMPLE}" "${ENV_FILE}"
  echo "Created env: .env from .env.example"
}

load_env() {
  set -a
  # shellcheck disable=SC1090
  . "${ENV_FILE}"
  set +a
  echo "Loaded env: .env"
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "Docker is required for the external docker-common infrastructure checks." >&2
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
  local common_stack_dir="${COMMON_STACK_DIR:-/path/to/docker-common}"
  cat <<EOF
Start or repair the shared stack first:
  cd ${common_stack_dir}
  docker compose up -d postgres redis rabbitmq etcd minio milvus attu neo4j
EOF
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

print_local_commands() {
  local backend_port="${HTTP_PORT:-62601}"
  cat <<EOF
Novex POC local process startup
-------------------------------
Shared infrastructure still runs in docker-common. Novex project processes run locally.

Run each command in a separate terminal from the repo root:

  (set -a; . .env; set +a; cargo run -p backend)
      # Backend: http://localhost:${backend_port}

  (set -a; . .env; set +a; EVAL_WORKER_ENABLED=true DB_AUTO_MIGRATE=false cargo run -p backend --bin eval_worker)
      # Eval worker

  (set -a; . .env; set +a; PARSER_BACKEND_BASE_URL=http://127.0.0.1:${backend_port} PARSER_BACKEND_TOKEN="\${PARSER_CALLBACK_TOKEN}" PYTHONPATH=services/parser-worker uv run --no-project --with-requirements services/parser-worker/requirements.txt python -m parser_worker.worker)
      # Parser worker with uv

  python3 -m venv services/parser-worker/.venv
  services/parser-worker/.venv/bin/python -m pip install -r services/parser-worker/requirements.txt
  (set -a; . .env; set +a; PARSER_BACKEND_BASE_URL=http://127.0.0.1:${backend_port} PARSER_BACKEND_TOKEN="\${PARSER_CALLBACK_TOKEN}" PYTHONPATH=services/parser-worker services/parser-worker/.venv/bin/python -m parser_worker.worker)
      # Parser worker fallback with .venv

  (set -a; . .env; set +a; cd admin && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:${backend_port} pnpm dev)
      # Admin: http://localhost:${ADMIN_PORT:-62602}

  (set -a; . .env; set +a; cd apps/training-web && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:${backend_port} pnpm dev)
      # Training Web: http://localhost:${TRAINING_WEB_PORT:-62603}

  (set -a; . .env; set +a; cd apps/notebooklm && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:${backend_port} pnpm dev)
      # NotebookLM: http://localhost:${NOTEBOOKLM_PORT:-62604}

  (set -a; . .env; set +a; cd apps/agent-workspace && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:${backend_port} pnpm dev)
      # Agent Workspace: http://localhost:${AGENT_WORKSPACE_PORT:-62605}

  (set -a; . .env; set +a; cd apps/codex-app-poc && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:${backend_port} pnpm dev)
      # Codex App POC: http://localhost:${CODEX_APP_POC_PORT:-62606}

  (set -a; . .env; set +a; cd apps/research-radar-poc && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:${backend_port} pnpm dev)
      # Research Radar POC: http://localhost:${RESEARCH_RADAR_POC_PORT:-62607}

Shared service URLs:
  RabbitMQ UI:   ${RABBITMQ_MANAGEMENT_URL:-http://localhost:15673}
  MinIO Console: ${MINIO_CONSOLE_URL:-http://localhost:19011}
  Attu:          ${ATTU_URL:-http://localhost:18000}
  Neo4j Browser: ${NEO4J_BROWSER_URL:-http://localhost:17474}
EOF
}

print_no_managed_processes() {
  cat <<'EOF'
Novex project services run as local terminal processes now.
Stop backend, eval-worker, parser-worker, and frontend apps with Ctrl-C in their terminals.

If old novex-poc Docker containers still exist, remove them by Compose project label:
  ids="$(docker ps -aq --filter label=com.docker.compose.project=novex-poc)"; [ -z "${ids}" ] || docker rm -f ${ids}
  vols="$(docker volume ls -q --filter label=com.docker.compose.project=novex-poc)"; [ -z "${vols}" ] || docker volume rm ${vols}
EOF
}

print_local_status_hint() {
  local backend_port="${HTTP_PORT:-62601}"
  local admin_port="${ADMIN_PORT:-62602}"
  local training_port="${TRAINING_WEB_PORT:-62603}"
  local notebooklm_port="${NOTEBOOKLM_PORT:-62604}"
  local agent_port="${AGENT_WORKSPACE_PORT:-62605}"
  local codex_port="${CODEX_APP_POC_PORT:-62606}"
  local research_radar_port="${RESEARCH_RADAR_POC_PORT:-62607}"
  cat <<EOF
Novex project services are local processes now.
Check the terminals where you started cargo/uv/venv/pnpm, or inspect ports:
  lsof -nP -iTCP:${backend_port} -sTCP:LISTEN
  lsof -nP -iTCP:${admin_port} -sTCP:LISTEN
  lsof -nP -iTCP:${training_port} -sTCP:LISTEN
  lsof -nP -iTCP:${notebooklm_port} -sTCP:LISTEN
  lsof -nP -iTCP:${agent_port} -sTCP:LISTEN
  lsof -nP -iTCP:${codex_port} -sTCP:LISTEN
  lsof -nP -iTCP:${research_radar_port} -sTCP:LISTEN
EOF
}

print_local_logs_hint() {
  cat <<'EOF'
Novex project service logs are printed in the local terminals where each process runs.
There is no managed novex-poc Docker log stream in the default local POC flow.
EOF
}

usage() {
  cat <<EOF
Usage: scripts/run-poc.sh [command]

Commands:
  check      Check docker-common, ensure the database, then print local startup commands
  up         Alias for check
  env        Check live AI environment variables only
  commands   Print local startup commands only
  status     Explain how to inspect local process status
  logs       Explain where local process logs are shown
  down       Explain how to stop local processes and clean old novex-poc containers
  help       Show this help

POC aggregate env entry:
  .env

Default local URLs:
  Backend          http://localhost:62601
  Admin            http://localhost:62602
  Training Web     http://localhost:62603
  NotebookLM       http://localhost:62604
  Agent Workspace  http://localhost:62605
  Codex App POC    http://localhost:62606
  Research Radar   http://localhost:62607
EOF
}

main "$@"

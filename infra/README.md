# Infra

Novex uses the shared `docker-common` infrastructure stack by default. Only shared infrastructure runs in Docker. Novex project services run as local processes: backend and eval-worker use Cargo, parser-worker uses uv or a local `.venv`, and frontend apps use pnpm.

Shared stack location:

```bash
/path/to/docker-common/docker-compose.yml
```

Shared stack service reference:

```bash
/path/to/docker-common/COMMON_DOCKER_README.md
```

## Shared Services

Start or repair the common stack first:

```bash
cd /path/to/docker-common
docker compose up -d postgres redis rabbitmq etcd minio milvus attu neo4j
```

Default host endpoints:

- PostgreSQL: `postgres://postgres:postgres@127.0.0.1:15432/novex`
- Redis: `redis://127.0.0.1:16379/0`
- RabbitMQ: `amqp://guest:guest@127.0.0.1:5673/%2f`, UI `http://localhost:15673`
- Milvus: `http://127.0.0.1:19540`
- MinIO: `http://127.0.0.1:19010`, console `http://localhost:19011`
- Attu: `http://localhost:18000`
- Neo4j: `bolt://127.0.0.1:17687`, browser `http://localhost:17474`

Local Novex processes use host endpoints. The shared `docker-common` containers still use these internal DNS names inside their own Docker network:

- PostgreSQL: `postgres:5432`
- Redis: `redis:6379`
- RabbitMQ: `rabbitmq:5672`
- Milvus: `milvus:19530`
- MinIO: `minio:9000`
- Neo4j: `neo4j:7687`

## Run

Check the shared stack and print local Novex startup commands:

```bash
./scripts/run-poc.sh
```

The script loads only `infra/.env.poc`, checks the `docker-common` containers, creates the `novex` PostgreSQL database when missing, checks live AI variables without printing raw secrets, and prints the local Cargo/uv or venv/pnpm commands. It does not create or start a `novex-poc` Docker Compose project.

Useful commands:

```bash
./scripts/run-poc.sh env
./scripts/run-poc.sh commands
./scripts/run-poc.sh status
./scripts/run-poc.sh logs
./scripts/run-poc.sh down
```

Run the backend and eval worker locally:

```bash
(set -a; . infra/.env.poc; set +a; cargo run -p backend)
(set -a; . infra/.env.poc; set +a; EVAL_WORKER_ENABLED=true DB_AUTO_MIGRATE=false cargo run -p backend --bin eval_worker)
```

Run the durable parser pipeline locally:

```bash
# uv, recommended
(set -a; . infra/.env.poc; set +a; PARSER_BACKEND_BASE_URL=http://127.0.0.1:4398 PARSER_BACKEND_TOKEN="${PARSER_CALLBACK_TOKEN}" PYTHONPATH=services/parser-worker uv run --no-project --with-requirements services/parser-worker/requirements.txt python -m parser_worker.worker)

# .venv fallback
python3 -m venv services/parser-worker/.venv
services/parser-worker/.venv/bin/python -m pip install -r services/parser-worker/requirements.txt
(set -a; . infra/.env.poc; set +a; PARSER_BACKEND_BASE_URL=http://127.0.0.1:4398 PARSER_BACKEND_TOKEN="${PARSER_CALLBACK_TOKEN}" PYTHONPATH=services/parser-worker services/parser-worker/.venv/bin/python -m parser_worker.worker)
```

The backend writes parser jobs to PostgreSQL outbox and publishes them to RabbitMQ. `parser-worker` consumes `novex.parser.execute`, coordinates in Redis, and callbacks the backend with `PARSER_CALLBACK_TOKEN`. Use a non-default token outside local POC.

Eval runs are outbox-backed. `POST /ai/evals/runs` creates `ai_eval_run`, `ai_eval_task`, and `ai_eval_outbox` rows. The backend publisher sends pending eval outbox rows to RabbitMQ, and `eval-worker` consumes `novex.eval.execute` to execute deterministic, `trace_replay`, and real `live_rag` tasks.

Run Admin and the customer-facing app templates locally with pnpm:

```bash
(cd admin && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 pnpm dev)
(cd apps/training-web && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 pnpm dev)
(cd apps/chat-web && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 pnpm dev)
(cd apps/agent-workspace && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 pnpm dev)
(cd apps/codex-app-poc && pnpm install && NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 pnpm dev)
```

Put local POC environment variables in `infra/.env.poc`:

```bash
$EDITOR infra/.env.poc
./scripts/run-poc.sh
```

`infra/.env.poc.example` is the committed schema/defaults file. Do not put secrets in the example file.

Live RAG and parser capability groups use these variables:

- LLM: `LLM_API_KEY`, `LLM_BASE_URL`, `LLM_MODEL`
- Embedding: `EMBEDDING_API_KEY`, `EMBEDDING_BASE_URL`, `EMBEDDING_MODEL`
- Reranker: `RERANKER_API_KEY`, `RERANKER_BASE_URL`, `RERANKER_MODEL`
- Draw: `RIGHT_CODE_DRAW_BASE_URL`, `RIGHT_CODE_DRAW_API_KEY`
- MinerU: `MINERU_TOKEN`, `PARSER_WORKER_MODE`, `MINERU_TIMEOUT_SECONDS`

## Smoke Checks

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
```

The local backend uses host endpoints such as `MILVUS_ENDPOINT=http://127.0.0.1:19540`, `RABBITMQ_URL=amqp://guest:guest@127.0.0.1:5673/%2f`, and `REDIS_URL=redis://127.0.0.1:16379/0`. Parser queue publishing is enabled in POC config through `PARSER_QUEUE_ENABLED=true` and `PARSER_QUEUE_PUBLISHER_ENABLED=true`. Eval queue publishing is enabled through `EVAL_QUEUE_ENABLED=true` and `EVAL_QUEUE_PUBLISHER_ENABLED=true`, with execution handled by local `eval-worker`. External GitHub, Feishu, draw, and MinerU credentials are optional. Without MinerU, text-like uploads still parse through the native parser path; PDF/Office/Image jobs stay retry/dead-letter governed by RabbitMQ.

# Infra

Local POC topology for Novex. The default compose path starts PostgreSQL, Milvus Standalone dependencies, RabbitMQ, Redis, Milvus, and the Rust backend with database migrations enabled. Admin and customer apps are under the `apps` profile, and the parser consumer is under the `parser` profile so the infrastructure stack can stay small during backend/RAG testing.

## Services

- `postgres`: PostgreSQL control-plane database on `localhost:5432`.
- `etcd`, `minio`, `milvus`: Milvus Standalone backing services on `localhost:19530`.
- `rabbitmq`: Durable parser and eval queue broker on `localhost:5672`, management UI on `localhost:15672`.
- `redis`: Parser worker lease/idempotency cache on `localhost:6379`.
- `backend`: Rust API on `localhost:4398`, with `DB_AUTO_MIGRATE=true`; publishes parser and eval outbox jobs.
- `eval-worker`: Rust eval queue consumer for real asynchronous eval tasks on `novex.eval.execute`.
- `parser-worker`: Python document parser queue consumer when the `parser` profile is enabled.
- `admin`: Admin control plane on `localhost:4399` when the `apps` profile is enabled.
- `training-web`: POC customer app on `localhost:4401` when the `apps` profile is enabled.
- `chat-web`: Knowledge/chat template on `localhost:4402` when the `apps` profile is enabled.
- `agent-workspace`: Agent run template on `localhost:4403` when the `apps` profile is enabled.

## Run

Start the full local POC stack with environment checks:

```bash
./scripts/run-poc.sh
```

The single local env entry for the POC stack is `infra/.env.poc`. The script creates it from `infra/.env.poc.example` when it is missing, loads only that file, checks live AI variables without printing raw secrets, verifies required Docker images already exist locally, then starts PostgreSQL, Milvus, RabbitMQ, Redis, the Rust backend, eval-worker, parser-worker, and all POC frontends. `up` runs with `--pull never`; use `./scripts/run-poc.sh pull` explicitly when you want to fetch only missing images.

Useful commands:

```bash
./scripts/run-poc.sh env
./scripts/run-poc.sh status
./scripts/run-poc.sh logs
./scripts/run-poc.sh down
./scripts/run-poc.sh pull
```

Equivalent minimal backend and eval stack:

```bash
docker compose --env-file infra/.env.poc -f infra/docker-compose.yml up postgres etcd minio milvus rabbitmq backend eval-worker
```

Run the durable parser pipeline:

```bash
docker compose --env-file infra/.env.poc -f infra/docker-compose.yml --profile parser up postgres etcd minio milvus rabbitmq redis backend parser-worker
```

The backend writes parser jobs to PostgreSQL outbox and publishes them to RabbitMQ. `parser-worker` consumes `novex.parser.execute`, coordinates in Redis, and callbacks the backend with `PARSER_CALLBACK_TOKEN`. Use a non-default token outside local POC.

Eval runs are outbox-backed. `POST /ai/evals/runs` creates `ai_eval_run`, `ai_eval_task`, and `ai_eval_outbox` rows. The backend publisher sends pending eval outbox rows to RabbitMQ, and `eval-worker` consumes `novex.eval.execute` to execute real `live_rag` tasks through the Novex knowledge path.

Run Admin and the customer-facing app templates:

```bash
docker compose --env-file infra/.env.poc -f infra/docker-compose.yml --profile apps up admin training-web chat-web agent-workspace
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

When a POC machine already has compatible local images, override compose image tags before starting the stack:

```bash
POSTGRES_IMAGE=postgres:16 \
MINIO_IMAGE=minio/minio:latest \
MILVUS_IMAGE=milvusdb/milvus:v2.6.0 \
RABBITMQ_IMAGE=rabbitmq:4.0-management-alpine \
REDIS_IMAGE=redis:7-alpine \
POSTGRES_PORT=55433 \
MINIO_API_PORT=9900 \
MINIO_CONSOLE_PORT=9901 \
docker compose --env-file infra/.env.poc -f infra/docker-compose.yml up postgres etcd minio milvus backend
```

Host port overrides do not change container-to-container addresses. The backend still connects to `postgres:5432`, `milvus:19530`, `rabbitmq:5672`, and `redis:6379` inside compose.

## Smoke Checks

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
```

The backend container sets `MILVUS_ENDPOINT=http://milvus:19530`; if Milvus is unavailable or no usable collection exists yet, the RAG path keeps the local hybrid fallback. Parser queue publishing is enabled in compose through `PARSER_QUEUE_ENABLED=true` and `PARSER_QUEUE_PUBLISHER_ENABLED=true`. Eval queue publishing is enabled through `EVAL_QUEUE_ENABLED=true` and `EVAL_QUEUE_PUBLISHER_ENABLED=true`, with execution handled by `eval-worker`. External GitHub, Feishu, draw, and MinerU credentials are optional. Without MinerU, text-like uploads still parse through the native parser path; PDF/Office/Image jobs stay retry/dead-letter governed by RabbitMQ.

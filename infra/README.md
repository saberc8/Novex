# Infra

Novex uses the shared `docker-common` infrastructure stack by default. The Novex compose file runs project services: backend, eval-worker, parser-worker, and frontend apps.

Shared stack location:

```bash
/Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom/docker-compose.yml
```

Shared stack service reference:

```bash
/Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom/COMMON_DOCKER_README.md
```

## Shared Services

Start or repair the common stack first:

```bash
cd /Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom
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

Novex containers join the external Docker network `docker-common_default` and use container DNS names:

- PostgreSQL: `postgres:5432`
- Redis: `redis:6379`
- RabbitMQ: `rabbitmq:5672`
- Milvus: `milvus:19530`
- MinIO: `minio:9000`
- Neo4j: `neo4j:7687`

## Run

Start the full Novex POC runtime:

```bash
./scripts/run-poc.sh
```

The script loads only `infra/.env.poc`, checks the `docker-common` containers, creates the `novex` PostgreSQL database when missing, checks live AI variables without printing raw secrets, verifies required Novex runtime images already exist locally, then starts backend, eval-worker, parser-worker, and the POC frontends. `up` runs with `--pull never`; use `./scripts/run-poc.sh pull` explicitly when you want to fetch only missing images.

Useful commands:

```bash
./scripts/run-poc.sh env
./scripts/run-poc.sh status
./scripts/run-poc.sh logs
./scripts/run-poc.sh down
./scripts/run-poc.sh pull
```

Minimal backend and eval runtime:

```bash
docker compose --env-file infra/.env.poc -f infra/docker-compose.yml up backend eval-worker
```

Run the durable parser pipeline:

```bash
docker compose --env-file infra/.env.poc -f infra/docker-compose.yml --profile parser up backend parser-worker
```

The backend writes parser jobs to PostgreSQL outbox and publishes them to RabbitMQ. `parser-worker` consumes `novex.parser.execute`, coordinates in Redis, and callbacks the backend with `PARSER_CALLBACK_TOKEN`. Use a non-default token outside local POC.

Eval runs are outbox-backed. `POST /ai/evals/runs` creates `ai_eval_run`, `ai_eval_task`, and `ai_eval_outbox` rows. The backend publisher sends pending eval outbox rows to RabbitMQ, and `eval-worker` consumes `novex.eval.execute` to execute deterministic, `trace_replay`, and real `live_rag` tasks.

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

## Smoke Checks

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
```

The backend and eval-worker containers set `MILVUS_ENDPOINT=http://milvus:19530`; host-run backend uses `MILVUS_ENDPOINT=http://127.0.0.1:19540`. Parser queue publishing is enabled in POC config through `PARSER_QUEUE_ENABLED=true` and `PARSER_QUEUE_PUBLISHER_ENABLED=true`. Eval queue publishing is enabled through `EVAL_QUEUE_ENABLED=true` and `EVAL_QUEUE_PUBLISHER_ENABLED=true`, with execution handled by `eval-worker`. External GitHub, Feishu, draw, and MinerU credentials are optional. Without MinerU, text-like uploads still parse through the native parser path; PDF/Office/Image jobs stay retry/dead-letter governed by RabbitMQ.

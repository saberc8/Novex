# Infra

Local POC topology for Novex. The default compose path starts PostgreSQL, Milvus Standalone dependencies, RabbitMQ, Redis, Milvus, and the Rust backend with database migrations enabled. Admin and customer apps are under the `apps` profile, and the parser consumer is under the `parser` profile so the infrastructure stack can stay small during backend/RAG testing.

## Services

- `postgres`: PostgreSQL control-plane database on `localhost:5432`.
- `etcd`, `minio`, `milvus`: Milvus Standalone backing services on `localhost:19530`.
- `rabbitmq`: Durable parser queue broker on `localhost:5672`, management UI on `localhost:15672`.
- `redis`: Parser worker lease/idempotency cache on `localhost:6379`.
- `backend`: Rust API on `localhost:4398`, with `DB_AUTO_MIGRATE=true`.
- `parser-worker`: Python document parser queue consumer when the `parser` profile is enabled.
- `admin`: Admin control plane on `localhost:4399` when the `apps` profile is enabled.
- `training-web`: POC customer app on `localhost:4401` when the `apps` profile is enabled.
- `chat-web`: Knowledge/chat template on `localhost:4402` when the `apps` profile is enabled.
- `agent-workspace`: Agent run template on `localhost:4403` when the `apps` profile is enabled.

## Run

```bash
docker compose -f infra/docker-compose.yml up postgres etcd minio milvus backend
```

Run the durable parser pipeline:

```bash
export PARSER_CALLBACK_TOKEN="local-parser-callback-token-change-me"
docker compose -f infra/docker-compose.yml --profile parser up postgres etcd minio milvus rabbitmq redis backend parser-worker
```

The backend writes parser jobs to PostgreSQL outbox and publishes them to RabbitMQ. `parser-worker` consumes `novex.parser.execute`, coordinates in Redis, and callbacks the backend with `PARSER_CALLBACK_TOKEN`. Use a non-default token outside local POC.

Run Admin and the customer-facing app templates:

```bash
docker compose -f infra/docker-compose.yml --profile apps up admin training-web chat-web agent-workspace
```

Optional live integration credentials can be exported from `infra/.env.poc.example` values before starting compose:

```bash
set -a
. infra/.env.poc
set +a
docker compose -f infra/docker-compose.yml --profile apps up
```

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
docker compose -f infra/docker-compose.yml up postgres etcd minio milvus backend
```

Host port overrides do not change container-to-container addresses. The backend still connects to `postgres:5432`, `milvus:19530`, `rabbitmq:5672`, and `redis:6379` inside compose.

## Smoke Checks

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
```

The backend container sets `MILVUS_ENDPOINT=http://milvus:19530`; if Milvus is unavailable or no usable collection exists yet, the RAG path keeps the local hybrid fallback. Parser queue publishing is enabled in compose through `PARSER_QUEUE_ENABLED=true` and `PARSER_QUEUE_PUBLISHER_ENABLED=true`. External GitHub, Feishu, draw, and MinerU credentials are optional. Without MinerU, text-like uploads still parse through the native parser path; PDF/Office/Image jobs stay retry/dead-letter governed by RabbitMQ.

# Infra

Local POC topology for Novex. The default compose path starts PostgreSQL, Milvus Standalone dependencies, Milvus, and the Rust backend with database migrations enabled. Admin and customer apps are under the `apps` profile so the infrastructure stack can stay small during backend/RAG testing.

## Services

- `postgres`: PostgreSQL control-plane database on `localhost:5432`.
- `etcd`, `minio`, `milvus`: Milvus Standalone backing services on `localhost:19530`.
- `backend`: Rust API on `localhost:4398`, with `DB_AUTO_MIGRATE=true`.
- `admin`: Admin control plane on `localhost:4399` when the `apps` profile is enabled.
- `training-web`: POC customer app on `localhost:4401` when the `apps` profile is enabled.
- `chat-web`: Knowledge/chat template on `localhost:4402` when the `apps` profile is enabled.
- `agent-workspace`: Agent run template on `localhost:4403` when the `apps` profile is enabled.

## Run

```bash
docker compose -f infra/docker-compose.yml up postgres etcd minio milvus backend
```

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
POSTGRES_PORT=55433 \
MINIO_API_PORT=9900 \
MINIO_CONSOLE_PORT=9901 \
docker compose -f infra/docker-compose.yml up postgres etcd minio milvus backend
```

Host port overrides do not change container-to-container addresses. The backend still connects to `postgres:5432` and `milvus:19530` inside compose.

## Smoke Checks

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
```

The backend container sets `MILVUS_ENDPOINT=http://milvus:19530`; if Milvus is unavailable or no usable collection exists yet, the RAG path keeps the local hybrid fallback. External GitHub, Feishu, and draw credentials are optional. Without them, the corresponding POC tools either use dry-run behavior or fall back to non-live runtime paths.

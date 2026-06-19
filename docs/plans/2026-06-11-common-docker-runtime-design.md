# Common Docker Runtime Design

## Goal

Novex should reuse the shared `docker-common` infrastructure stack by default, so future projects do not each own a separate PostgreSQL, Redis, MinIO, Milvus, Etcd, Neo4j, or RabbitMQ stack.

## Current Context

The shared Docker stack lives at:

- `/path/to/docker-common/docker-compose.yml`

It already provides PostgreSQL, Redis, Etcd, MinIO, Milvus, Attu, and Neo4j through the `docker-common` Compose project and the `docker-common_default` network. Novex keeps project-local runtime defaults in `.env.example`, `README.md`, `scripts/run-poc.sh`, and backend local POC contract tests.

## Architecture

The shared stack becomes the owner of infrastructure containers. Add RabbitMQ to that stack with the same naming convention as existing services: `docker-rabbitmq`, configurable host ports, healthcheck, and a named persistent volume.

Novex project services run as local processes. They connect to common services through host ports while the common containers still use network DNS names such as `postgres`, `redis`, `milvus`, and `rabbitmq` inside the shared Docker network.

Local host execution uses the common host ports:

- PostgreSQL: `127.0.0.1:15432`
- Redis: `127.0.0.1:16379`
- Milvus: `127.0.0.1:19540`
- MinIO: `127.0.0.1:19010` and `127.0.0.1:19011`
- Neo4j: `127.0.0.1:17474` and `127.0.0.1:17687`
- RabbitMQ: host `127.0.0.1:5673` and `127.0.0.1:15673`; container `rabbitmq:5672`

## Runtime Flow

1. Start or update the common stack from the aether-loom compose file.
2. `docker-rabbitmq` starts with management UI and healthcheck.
3. Novex `run-poc.sh` checks that required common containers are available instead of launching infrastructure containers itself.
4. Novex backend and parser-worker connect to shared services through the external common network.
5. Host-run Novex commands read `.env` or `.env` and use the common host ports.

## Error Handling

Novex startup should fail early when the required common network or containers are missing. The error should point users to the common compose command instead of silently starting project-local infrastructure.

## Testing

Use static compose/config validation plus live Docker checks:

- `docker compose -p docker-common -f <common-compose> config`
- `docker compose -p docker-common -f <common-compose> up -d rabbitmq`
- `docker ps` to confirm `docker-rabbitmq` is running and healthy
- `./scripts/run-poc.sh commands`
- targeted Novex backend tests that assert the local POC contract

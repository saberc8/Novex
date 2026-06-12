# Common Docker Runtime Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move Novex to the shared `docker-common` infrastructure stack and add RabbitMQ to that common stack.

**Architecture:** Common Docker owns infrastructure containers and persistent volumes. Novex Compose owns only project runtime containers and joins the external `docker-common_default` network for service discovery.

**Tech Stack:** Docker Compose v2, PostgreSQL/pgvector, Redis, Etcd, MinIO, Milvus, Attu, Neo4j, RabbitMQ, Rust backend, Python parser-worker, Next.js apps.

---

### Task 1: Add RabbitMQ To Common Docker

**Files:**
- Modify: `/Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom/docker-compose.yml`
- Modify: `/Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom/.env.example`
- Modify: `/Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom/README.md`
- Create: `/Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom/COMMON_DOCKER_README.md`

**Steps:**
1. Add `rabbitmq` service with `rabbitmq:4.0-management-alpine`, `docker-rabbitmq`, `DOCKER_RABBITMQ_AMQP_PORT`, `DOCKER_RABBITMQ_MANAGEMENT_PORT`, `RABBITMQ_DEFAULT_USER`, `RABBITMQ_DEFAULT_PASS`, healthcheck, and `docker-rabbitmq-data`.
2. Add RabbitMQ defaults to `.env.example`.
3. Update README local infrastructure commands and defaults.
4. Add a concise common service reference README with host URLs, container URLs, usernames, and passwords.
5. Run common compose config validation.
6. Start RabbitMQ and verify container health.

### Task 2: Move Novex Compose To Common Infrastructure

**Files:**
- Modify: `Novex/infra/docker-compose.yml`
- Modify: `Novex/infra/.env.poc.example`
- Modify: `Novex/infra/.env.poc`

**Steps:**
1. Remove project-local PostgreSQL, Etcd, MinIO, Milvus, RabbitMQ, and Redis services from Novex compose.
2. Attach Novex runtime services to external network `docker-common_default`.
3. Change container runtime env to use common service DNS names.
4. Change host runtime env examples to use common host ports.
5. Keep local secrets only in ignored `.env.poc`.
6. Run Novex compose config validation.

### Task 3: Update Novex Run Script And Docs

**Files:**
- Modify: `Novex/scripts/run-poc.sh`
- Modify: `Novex/infra/README.md`
- Modify: `Novex/backend/.env.example`
- Modify: `Novex/backend/.env`

**Steps:**
1. Replace project infrastructure services in `run-poc.sh` with a common-service prerequisite check.
2. Keep `backend`, `parser-worker`, `admin`, `training-web`, `chat-web`, and `agent-workspace` as Novex services.
3. Update printed URLs to show common RabbitMQ and MinIO management addresses.
4. Update docs and env samples to explain common service defaults.
5. Ensure local host backend defaults use common ports.

### Task 4: Update Contract Tests And Verify

**Files:**
- Modify: `Novex/backend/src/interfaces/http/ai/foundation.rs`

**Steps:**
1. Update compose contract assertions to expect only Novex project runtime services.
2. Add assertions for external common network and common service connection strings.
3. Run `cargo test -p backend-rust local_poc_compose_declares_foundation_runtime_services`.
4. Run Docker compose config checks for common and Novex.
5. Run `docker ps` to confirm the common stack includes healthy RabbitMQ.


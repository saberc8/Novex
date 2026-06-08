# Parser Queue Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a durable RabbitMQ/Redis-backed parser execution pipeline so knowledge file upload automatically parses, indexes, and becomes queryable.

**Architecture:** Add a PostgreSQL outbox row when parser jobs are created, publish outbox events to a dedicated RabbitMQ parser exchange, and run parser-worker as a long-lived consumer. Redis provides short-lived worker leases, idempotency, retry counters, and heartbeat cache while PostgreSQL remains the source of truth.

**Tech Stack:** Rust backend-rust, SQLx/PostgreSQL migrations, lapin/RabbitMQ, redis-rs or Python redis client, Python parser-worker, Next.js training-web, Vitest, Rust unit tests, Python unittest.

---

### Task 1: Parser Outbox Schema

**Files:**
- Create: `backend/migrations/202606080001_create_ai_parser_outbox.sql`
- Modify: `backend/src/infrastructure/persistence/ai_knowledge_repository.rs`
- Test: `backend/src/infrastructure/persistence/ai_knowledge_repository.rs`

**Steps:**
1. Write a failing Rust test that asserts the migration contains `ai_parser_outbox`, parser job indexes, unique parser job idempotency, status, attempt count, and payload JSONB.
2. Run `cargo test -p backend-rust infrastructure::persistence::ai_knowledge_repository::tests::parser_outbox_migration_defines_durable_queue_contract`.
3. Add the migration and repository string assertions.
4. Re-run the test and confirm it passes.

### Task 2: Insert Outbox With Parse Job Transaction

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_knowledge_repository.rs`
- Modify: `backend/src/application/ai/knowledge_service.rs`
- Test: `backend/src/infrastructure/persistence/ai_knowledge_repository.rs`
- Test: `backend/src/application/ai/knowledge_service.rs`

**Steps:**
1. Write failing tests that assert `create_document_parse_job` inserts `ai_parser_outbox` in the same transaction and that `parser_worker_request` becomes the outbox payload.
2. Run targeted tests and verify failure.
3. Add `ParserOutboxSaveRecord`, `parser_outbox_save_record`, and an insert call inside `create_document_parse_job`.
4. Re-run targeted tests and confirm pass.

### Task 3: Parser RabbitMQ Types And Topology

**Files:**
- Modify: `backend/src/infrastructure/mq/rabbitmq.rs`
- Test: `backend/src/infrastructure/mq/rabbitmq.rs`

**Steps:**
1. Write failing tests for `ParserJobMessage` camelCase serialization and default parser queue config.
2. Run targeted tests and verify failure.
3. Add parser message/config structs and generic publish helpers while keeping scheduler behavior unchanged.
4. Re-run targeted tests and confirm pass.

### Task 4: Backend Parser Queue Config

**Files:**
- Modify: `backend/src/shared/config.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Create: `backend/src/application/ai/parser_queue_runtime.rs`
- Test: `backend/src/shared/config.rs`
- Test: `backend/src/application/ai/parser_queue_runtime.rs`

**Steps:**
1. Write failing tests for parser queue env defaults, truthy flags, and conversion into RabbitMQ config.
2. Run targeted tests and verify failure.
3. Add config fields and a small runtime config conversion layer.
4. Re-run tests and confirm pass.

### Task 5: Outbox Publisher

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_knowledge_repository.rs`
- Create/Modify: `backend/src/application/ai/parser_queue_runtime.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/src/application/ai/parser_queue_runtime.rs`

**Steps:**
1. Write failing tests for publisher behavior using a fake publisher: pending outbox rows produce parser messages and rows mark published only after fake ack.
2. Run tests and verify failure.
3. Add repository methods `list_pending_parser_outbox`, `mark_parser_outbox_published`, and `mark_parser_outbox_publish_failed`.
4. Add `publish_pending_parser_jobs` with an injectable publisher trait.
5. Wire optional runtime spawn in `main.rs`.
6. Re-run tests and confirm pass.

### Task 6: Python Worker Message Model And Redis Lease

**Files:**
- Create: `services/parser-worker/parser_worker/worker.py`
- Test: `services/parser-worker/tests/test_worker.py`

**Steps:**
1. Write failing Python tests for parsing RabbitMQ messages, acquiring/releasing fake Redis lease, and skipping duplicate idempotency keys.
2. Run `PYTHONPATH=services/parser-worker python3 -m unittest services/parser-worker/tests/test_worker.py` and verify failure.
3. Implement message dataclass/helpers and Redis lease/idempotency functions with injectable clients.
4. Re-run tests and confirm pass.

### Task 7: Python Worker Retry And Dead Routing

**Files:**
- Modify: `services/parser-worker/parser_worker/worker.py`
- Test: `services/parser-worker/tests/test_worker.py`

**Steps:**
1. Write failing tests for native parse success ack, MinerU submitted retry publish, backend callback failure retry, and exhausted attempts dead-letter publish.
2. Run targeted tests and verify failure.
3. Implement `handle_parser_message` with injectable `runner`, `publisher`, and fake Redis.
4. Re-run tests and confirm pass.

### Task 8: Worker Runtime Entrypoint

**Files:**
- Modify: `services/parser-worker/parser_worker/worker.py`
- Modify: `services/parser-worker/README.md`
- Test: `services/parser-worker/tests/test_worker.py`

**Steps:**
1. Write failing tests for env-based worker config masking and queue names.
2. Run tests and verify failure.
3. Add RabbitMQ/Redis connection setup and a `main()` loop guarded behind imports so unit tests stay offline.
4. Update README with run commands.
5. Re-run parser-worker tests.

### Task 9: Training-Web Polling

**Files:**
- Modify: `apps/training-web/src/api/knowledge.ts`
- Modify: `apps/training-web/src/types/knowledge.ts`
- Modify: `apps/training-web/src/app-client.tsx`
- Test: `apps/training-web/src/api/knowledge.test.ts`
- Test: `apps/training-web/app/page.test.tsx`

**Steps:**
1. Write failing tests for `getParseJob` API wrapper and upload UI polling until indexed.
2. Run targeted Vitest tests and verify failure.
3. Add API wrapper, UI polling state, indexed/failed status display, and dataset refresh after indexed.
4. Re-run targeted frontend tests.

### Task 10: Docker Compose Wiring

**Files:**
- Modify: `infra/docker-compose.yml`
- Modify: `infra/README.md`
- Test: `infra/README.md` or backend config tests where applicable

**Steps:**
1. Write/extend text assertions in an existing Rust test or add lightweight shell-verifiable docs checks for RabbitMQ, Redis, parser-worker env names.
2. Run the check and verify failure.
3. Add RabbitMQ, Redis, parser-worker services and env wiring.
4. Re-run checks.

### Task 11: End-To-End Text Upload Acceptance

**Files:**
- Create: `backend/tests/parser_queue_text_e2e.rs` or add a focused service-level test where DB is available.
- Modify: docs if needed.

**Steps:**
1. Write a gated/integration test that creates a dataset, simulates file parser message completion, verifies indexed chunks, and asks a question that returns citation.
2. Run it and verify failure before final plumbing.
3. Finish any missing integration hooks.
4. Re-run targeted test.

### Task 12: Full Verification

**Commands:**
- `cargo test -p backend-rust application::ai::knowledge_service::tests infrastructure::mq::rabbitmq::tests`
- `cargo test -p backend-rust interfaces::http::ai::knowledge::tests`
- `PYTHONPATH=services/parser-worker python3 -m unittest discover -s services/parser-worker/tests`
- `pnpm test src/api/knowledge.test.ts app/page.test.tsx` in `apps/training-web`
- `cargo test --workspace` if time allows

**Expected:** All non-live tests pass. Live MinerU/RabbitMQ/Redis smoke remains gated by explicit env flags.

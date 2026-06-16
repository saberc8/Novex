# Agent Event Stream Implementation Plan

**Goal:** Add a Codex-style replayable event stream for Agent runs on top of the durable Run Graph event table.

**Architecture:** Backend exposes an SSE endpoint with a `sequence_no` cursor. Repository and service provide cursor reads; HTTP owns the polling stream. Frontend helpers use `fetch` with bearer auth for `text/event-stream`.

## Task 1: Backend Stream Contract Tests

Files:

- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/interfaces/http/ai/agent.rs`

Add tests:

- stream query normalization clamps `afterSequenceNo`, `batchSize`, `pollMs`, and `maxIdleMs`,
- repository source includes `sequence_no > $3` cursor filtering and ordered limited query,
- service source exposes cursor event listing and terminal run status,
- HTTP route source includes `/events/stream`, `Sse`, and `KeepAlive`,
- missing permission on the stream handler returns `Forbidden`,
- SSE event builder emits `agent_run_event` and uses `sequence_no` as the id.

Run:

```bash
cargo test -p backend-rust agent_event_stream --offline
```

Expected: FAIL until the stream contract exists.

## Task 2: Backend SSE Implementation

Files:

- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/interfaces/http/ai/agent.rs`

Implement:

- `RunEventCursorFilter`
- `AiAgentRepository::list_events_after_sequence`
- `AgentRunEventStreamQuery` and normalized settings
- `AgentService::list_events_after_sequence`
- `AgentService::is_run_terminal`
- `GET /ai/agents/runs/:run_id/events/stream`
- SSE polling loop with keepalive, terminal close, and one-shot error event.

Run:

```bash
cargo test -p backend-rust agent_event_stream --offline
cargo test -p backend-rust agent_event_list --offline
```

Expected: PASS.

## Task 3: Frontend API Helper

Files:

- Modify: `apps/agent-workspace/src/types/agent.ts`
- Modify: `apps/agent-workspace/src/api/agent.ts`
- Modify: `apps/agent-workspace/src/api/agent.test.ts`
- Modify: `apps/codex-app-poc/src/types/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.test.ts`

Implement:

- stream query type,
- `fetchAgentRunEventStream(runId, query?)`,
- tests proving `Accept: text/event-stream`, bearer auth where available, and query parameters.

Run:

```bash
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

Expected: PASS.

## Task 4: Matrix And Verification

Files:

- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

Update:

- Runtime loop or Agent protocol row with event stream progress.
- Current acceptance evidence with backend and frontend commands.

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

Then merge feature into `main` and rerun the same verification on `main`.

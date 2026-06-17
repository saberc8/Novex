# Agent Event WebSocket Browser Auth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a browser-safe, short-lived ticket handoff for agent run-event WebSocket connections.

**Architecture:** Reuse the existing JWT signing infrastructure and existing WebSocket event stream loop. Add a purpose/run-bound ticket endpoint and a pre-upgrade principal extractor that accepts either `Authorization` or `?ticket=...`.

**Tech Stack:** Rust, axum extractors, existing `JwtService`, existing `AgentService`, Next/Vitest frontend API helpers.

## Global Constraints

- TDD: write failing tests first and verify red before production code.
- Ticket TTL is 60 seconds.
- Ticket purpose is exactly `agent_run_event_ws`.
- Ticket is scoped to a single `run_id`.
- Do not persist tickets in this slice.
- Do not add provider token-delta streaming in this slice.
- Do not mark the persistent goal complete after this slice.
- Merge the feature worktree back to main after verification.
- Run `cargo clean` in both worktrees after the stage completes.

---

### Task 1: JWT Ticket Contract

**Files:**
- Modify: `backend/src/infrastructure/security/jwt.rs`
- Modify: `docs/plans/2026-06-17-agent-event-websocket-browser-auth.md`

**Interfaces:**
- Adds: `AgentRunEventWsTicketClaims`
- Adds: `JwtService::issue_agent_run_event_ws_ticket(user_id, username, run_id, ttl_seconds) -> Result<IssuedToken>`
- Adds: `JwtService::parse_agent_run_event_ws_ticket(ticket, expected_run_id) -> Result<AgentRunEventWsTicketClaims>`

- [ ] **Step 1: Write failing tests**

Add tests proving a ticket round-trips and is rejected for a different run id.

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust agent_event_ws_ticket --offline
```

Expected: FAIL because the ticket methods do not exist.

- [ ] **Step 3: Implement minimal ticket claims and methods**

Use the existing JWT secret, include `purpose`, `user_id`, `username`, `run_id`, `iat`, and `exp`, and validate purpose plus run id when parsing.

- [ ] **Step 4: Run green test**

Run:

```bash
cargo test -p backend-rust agent_event_ws_ticket --offline
```

Expected: PASS.

### Task 2: Backend Ticket Endpoint and WebSocket Principal

**Files:**
- Modify: `backend/src/interfaces/http/ai/agent.rs`

**Interfaces:**
- Adds: `POST /ai/agents/runs/:run_id/events/ws-ticket`
- Adds: `AgentRunEventWsTicketResp { ticket, expiresInSeconds }`
- Adds: `AgentRunEventWsPrincipal`

- [ ] **Step 1: Write failing tests**

Add tests proving:

- The ticket endpoint route exists and requires auth.
- The ticket handler rejects missing `ai:agent:event:list`.
- Source contract includes `AgentRunEventWsPrincipal` before `WebSocketUpgrade`.
- Source contract includes `parse_agent_run_event_ws_ticket`.

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust agent_event_websocket_ticket --offline
```

Expected: FAIL because the route, response, and principal extractor do not exist.

- [ ] **Step 3: Implement endpoint and extractor**

Add the route and handler. The extractor reads `Authorization` first, then query `ticket`; ticket auth carries a `ticket_run_id` that must match the path `run_id`.

- [ ] **Step 4: Run green backend tests**

Run:

```bash
cargo test -p backend-rust agent_event_websocket_ticket --offline
cargo test -p backend-rust agent_event_websocket --offline
```

Expected: PASS.

### Task 3: Browser API Helpers

**Files:**
- Modify: `apps/agent-workspace/src/api/agent.ts`
- Modify: `apps/agent-workspace/src/api/agent.test.ts`
- Modify: `apps/codex-app-poc/src/lib/api.ts`
- Create: `apps/codex-app-poc/src/lib/auth.ts`
- Modify: `apps/codex-app-poc/src/api/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.test.ts`

**Interfaces:**
- Adds: `createAgentRunEventWebSocketTicket(runId)`
- Adds: `agentRunEventWebSocketUrl(runId, ticket, query)`

- [ ] **Step 1: Write failing frontend tests**

Add tests proving ticket requests use normal HTTP bearer auth and WS URL helpers convert protocol and include cursor query plus ticket.

- [ ] **Step 2: Run red frontend tests**

Run:

```bash
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd ../codex-app-poc && pnpm test -- src/api/agent.test.ts
```

Expected: FAIL because helpers do not exist.

- [ ] **Step 3: Implement helpers**

Reuse each app's `apiUrl` helper. In `codex-app-poc`, add the same `novex_token` localStorage helper and attach `Authorization` in `apiRequest`.

- [ ] **Step 4: Run green frontend tests**

Run:

```bash
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd ../codex-app-poc && pnpm test -- src/api/agent.test.ts
```

Expected: PASS.

### Task 4: Matrix, Verification, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-event-websocket-browser-auth.md`

- [ ] **Step 1: Update migration matrix**

Move browser WebSocket token handoff into Runtime loop implemented evidence. Keep provider token-delta streaming as remaining work.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd ../codex-app-poc && pnpm test -- src/api/agent.test.ts
git diff --check
```

Expected: PASS.

- [ ] **Step 3: Commit feature branch**

Commit with:

```bash
git commit -m "feat: add agent websocket browser tickets"
```

- [ ] **Step 4: Merge to main**

Merge with:

```bash
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation websocket browser tickets"
```

- [ ] **Step 5: Verify main and clean**

Run the full verification on main, then run `cargo clean` in both main and feature worktrees and fast-forward the feature branch to main.

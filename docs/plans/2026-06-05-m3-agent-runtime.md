# M3 Agent Runtime Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a minimal Codex-like Agent Runtime POC with Run Graph state, ReAct/tool-loop events, approval pause/resume/cancel, event snapshot, trace IDs, and task budget enforcement.

**Architecture:** Keep Run Graph state transition types in `novex-ai-core` and deterministic agent planning in `novex-agent`. Keep HTTP, RBAC, migrations, persistence, and dry-run orchestration in `backend`. Reuse the M2 tool registry and audit table for tool metadata and tool-call audit; M3 does not execute real external side effects.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL migrations, Next.js Admin, Vitest.

---

### Task 1: Run Graph and Agent Domain Kernel

**Files:**
- Modify: `crates/novex-ai-core/src/lib.rs`
- Modify: `crates/novex-agent/src/lib.rs`

**Step 1: Write failing tests**

Add tests for:

- Valid Run Graph status transitions: `queued -> running -> waiting_approval -> resuming -> running -> succeeded`.
- Cancel transition from active states to `cancelling -> cancelled`.
- Invalid terminal transition from `succeeded` back to `running` is rejected.
- Agent intent routing maps knowledge questions, tool requests, training quiz requests, and human handoff requests.
- Tool selector chooses `rag.search`, `media.image.generate`, or `feishu.message.send` from deterministic input text.
- Task budget rejects a run when requested max steps/tool calls exceed configured POC limits.

Run:

```bash
cargo test -p novex-ai-core run_graph --offline
cargo test -p novex-agent agent_runtime --offline
```

Expected: fail because the new transition helpers and planner APIs do not exist.

**Step 2: Implement minimal domain APIs**

Add to `novex-ai-core`:

- `RunTransitionError`
- `RunStatus::is_terminal`
- `can_transition_run_status(from, to)`
- `validate_run_transition(from, to)`
- `PauseReason`
- `RunEventKind`
- `TaskBudget`
- `normalize_task_budget`

Add to `novex-agent`:

- `AgentRunPlan`
- `SelectedTool`
- `route_intent(input: &str) -> AgentIntent`
- `select_tool(input: &str) -> Option<SelectedTool>`
- `plan_react_run(input: &str, budget: TaskBudget) -> Result<AgentRunPlan, AgentPlanError>`

**Step 3: Verify and commit**

```bash
cargo test -p novex-ai-core --offline
cargo test -p novex-agent --offline
cargo test --workspace --offline
git add crates/novex-ai-core/src/lib.rs crates/novex-agent/src/lib.rs
git commit -m "feat: add agent run graph domain kernel"
```

### Task 2: Run Graph Runtime Schema and Permissions

**Files:**
- Create: `backend/migrations/202606050008_create_ai_agent_runtime.sql`
- Create: `backend/migrations/202606050009_seed_ai_agent_permissions.sql`

**Step 1: Add schema and permissions**

Create tables:

- `ai_run`
- `ai_run_step`
- `ai_run_event`
- `ai_run_pause`
- `ai_agent_run`
- `ai_agent_trace`

Seed permissions:

- `ai:agent:run`
- `ai:agent:event:list`
- `ai:agent:resume`
- `ai:agent:cancel`

All tables must include `tenant_id`, timestamps, status, and stable IDs. `ai_agent_run` and `ai_agent_trace` must reference `run_id` instead of owning a second status machine.

**Step 2: Verify**

```bash
rg "CREATE TABLE IF NOT EXISTS ai_run|CREATE TABLE IF NOT EXISTS ai_run_event|CREATE TABLE IF NOT EXISTS ai_run_pause" backend/migrations/202606050008_create_ai_agent_runtime.sql
rg "ai:agent:run|ai:agent:event:list|ai:agent:resume|ai:agent:cancel" backend/migrations/202606050009_seed_ai_agent_permissions.sql
cargo test -p backend --offline
```

**Step 3: Commit**

```bash
git add backend/migrations/202606050008_create_ai_agent_runtime.sql backend/migrations/202606050009_seed_ai_agent_permissions.sql
git commit -m "feat: add ai agent runtime schema"
```

### Task 3: Backend Agent Runtime API

**Files:**
- Create: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Create: `backend/src/application/ai/agent_service.rs`
- Create: `backend/src/interfaces/http/ai/agent.rs`
- Modify: `backend/src/infrastructure/persistence/mod.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`

**Step 1: Write failing tests**

Add tests for:

- Blank run input is rejected.
- Low-risk `rag.search` run succeeds and writes run events.
- Medium-risk `feishu.message.send` run pauses with `waiting_approval` when `autoApprove=false`.
- Resuming an approval run writes resumed/tool/final events and succeeds.
- Cancel moves an active run to `cancelled` and prevents future tool execution.
- Event snapshot returns ordered events by sequence number.
- Route registration requires authentication and permissions match seeded codes.

Run:

```bash
cargo test -p backend agent_runtime --offline
```

Expected: fail because backend agent modules and routes do not exist.

**Step 2: Implement service and repository**

Add endpoints:

- `POST /ai/agents/runs`
- `GET /ai/agents/runs`
- `GET /ai/agents/runs/:run_id`
- `GET /ai/agents/runs/:run_id/events`
- `POST /ai/agents/runs/:run_id/resume`
- `POST /ai/agents/runs/:run_id/cancel`

Runtime behavior:

- Create `ai_run`, `ai_agent_run`, `ai_agent_trace`, and ordered `ai_run_event` rows.
- Use `novex-agent` deterministic planning for intent and tool selection.
- Use M2 `ai_tool` metadata for risk and permission.
- For low-risk tools or `autoApprove=true`, create a dry-run tool audit and finish the run.
- For medium/high-risk tools with `autoApprove=false`, create an `approval` step, `ai_run_pause`, and `waiting_approval` event.
- `resume` closes the active pause, writes `resumed`, performs dry-run execution, and finishes.
- `cancel` writes `cancelling` and `cancelled` events and sets terminal status.
- `events` acts as the event snapshot for reconnecting UI.

**Step 3: Verify and commit**

```bash
cargo test -p backend agent_runtime --offline
cargo test -p backend --offline
git add backend/src/infrastructure/persistence/ai_agent_repository.rs backend/src/application/ai/agent_service.rs backend/src/interfaces/http/ai/agent.rs backend/src/infrastructure/persistence/mod.rs backend/src/application/ai/mod.rs backend/src/interfaces/http/ai/mod.rs
git commit -m "feat: add agent runtime api"
```

### Task 4: Admin Agent Runtime Console

**Files:**
- Create: `admin/src/types/ai-agent.ts`
- Create: `admin/src/api/ai/agent.ts`
- Create: `admin/src/api/ai/agent.test.ts`
- Modify: `admin/app/(main)/ai/agents/page.tsx`

**Step 1: Write failing tests**

Test API wrappers for:

- `createAgentRun`
- `listAgentRuns`
- `getAgentRun`
- `listAgentRunEvents`
- `resumeAgentRun`
- `cancelAgentRun`

Run:

```bash
pnpm vitest run src/api/ai/agent.test.ts
```

Expected: fail because wrappers do not exist.

**Step 2: Implement Admin console**

Replace the Agent placeholder with:

- Compact run command form with input, tool loop budget, and auto-approve toggle.
- Run list showing status, intent, selected tool, pause reason, and trace ID.
- Event snapshot panel showing ordered event kind, status, and payload.
- Resume approval and cancel buttons gated by `ai:agent:resume` and `ai:agent:cancel`.

**Step 3: Verify and commit**

```bash
pnpm typecheck
pnpm vitest run src/api/ai/agent.test.ts
pnpm lint
git add admin/src/types/ai-agent.ts admin/src/api/ai/agent.ts admin/src/api/ai/agent.test.ts 'admin/app/(main)/ai/agents/page.tsx'
git commit -m "feat: add agent runtime admin console"
```

### Task 5: M3 Verification and Smoke

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd admin && pnpm typecheck && pnpm vitest run && pnpm lint && pnpm build
```

After merging to `main`, restart backend with `DB_AUTO_MIGRATE=true`, then smoke:

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
curl http://localhost:4399/ai/agents
```

With an admin JWT:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"input":"send Feishu training reminder","autoApprove":false,"budget":{"maxSteps":6,"maxToolCalls":2}}' \
  http://localhost:4398/ai/agents/runs

curl -H "Authorization: Bearer $TOKEN" http://localhost:4398/ai/agents/runs/{runId}/events
curl -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{"approved":true}' http://localhost:4398/ai/agents/runs/{runId}/resume
curl -H "Authorization: Bearer $TOKEN" http://localhost:4398/ai/agents/runs/{runId}/events
```

# M0 Foundation Skeleton Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the M0 Novex foundation skeleton with compileable Rust workspace crates, sidecar/app/template directories, AI admin menu entries, and a minimal backend AI foundation summary endpoint.

**Architecture:** Keep the existing RBAC backend as the control plane and move reusable AI foundation boundaries into independent Rust workspace crates. The backend may read stable crate metadata, but AI domain logic must remain outside `backend/src`.

**Tech Stack:** Rust 2021, Cargo workspace, Axum, SQLx, PostgreSQL migrations, Next.js 16, React 19, TypeScript, Tailwind.

---

### Task 1: Add Rust Workspace And Core Crate Metadata

**Files:**
- Create: `Cargo.toml`
- Create: `crates/novex-ai-core/Cargo.toml`
- Create: `crates/novex-ai-core/src/lib.rs`
- Modify: `backend/Cargo.toml`
- Test: `crates/novex-ai-core/src/lib.rs`

**Step 1: Write failing crate tests**

Create tests in `crates/novex-ai-core/src/lib.rs` that assert:

```rust
let modules = foundation_modules();
assert!(modules.iter().any(|module| module.id == "run-graph"));
assert!(modules.iter().any(|module| module.id == "policy"));
assert!(modules.iter().all(|module| module.status == FoundationStatus::Skeleton));
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p novex-ai-core`

Expected: FAIL because workspace and crate do not exist yet.

**Step 3: Add minimal implementation**

Create root workspace `Cargo.toml`, add `backend` and `crates/novex-ai-core` as members, and implement:

- `FoundationStatus`
- `FoundationModule`
- `TenantContext`
- `ResourceRef`
- `RunStatus`
- `RunStepType`
- `foundation_modules()`

**Step 4: Run test to verify it passes**

Run: `cargo test -p novex-ai-core`

Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml backend/Cargo.toml crates/novex-ai-core
git commit -m "feat: add novex ai core workspace crate"
```

### Task 2: Add Remaining AI Crate Skeletons

**Files:**
- Create: `crates/novex-model/Cargo.toml`
- Create: `crates/novex-model/src/lib.rs`
- Create: `crates/novex-rag/Cargo.toml`
- Create: `crates/novex-rag/src/lib.rs`
- Create: `crates/novex-agent/Cargo.toml`
- Create: `crates/novex-agent/src/lib.rs`
- Create: `crates/novex-tools/Cargo.toml`
- Create: `crates/novex-tools/src/lib.rs`
- Create: `crates/novex-connectors/Cargo.toml`
- Create: `crates/novex-connectors/src/lib.rs`
- Create: `crates/novex-mcp/Cargo.toml`
- Create: `crates/novex-mcp/src/lib.rs`
- Create: `crates/novex-plugin/Cargo.toml`
- Create: `crates/novex-plugin/src/lib.rs`
- Create: `crates/novex-trigger/Cargo.toml`
- Create: `crates/novex-trigger/src/lib.rs`
- Create: `crates/novex-memory/Cargo.toml`
- Create: `crates/novex-memory/src/lib.rs`
- Create: `crates/novex-eval/Cargo.toml`
- Create: `crates/novex-eval/src/lib.rs`
- Modify: `Cargo.toml`

**Step 1: Write failing tests**

Each crate should include one unit test asserting its `module()` returns the expected module ID and `FoundationStatus::Skeleton`.

**Step 2: Run tests to verify they fail**

Run: `cargo test --workspace`

Expected: FAIL because crates do not exist or are not members yet.

**Step 3: Add minimal implementation**

Add each crate with small domain boundary types, for example:

- `novex-model`: `ModelKind`, `ModelProviderType`, `ModelRoutePurpose`
- `novex-rag`: `KnowledgeResourceKind`, `RetrievalMode`, `CitationRef`
- `novex-agent`: `AgentIntent`, `AgentLoopKind`
- `novex-tools`: `ToolKind`, `ToolRiskLevel`, `ApprovalPolicy`
- `novex-connectors`: `ConnectorKind`, `CredentialScope`
- `novex-mcp`: `McpServerStatus`, `McpToolDescriptor`
- `novex-plugin`: `PluginRuntime`, `PluginCapabilityKind`
- `novex-trigger`: `TriggerSourceKind`, `TriggerTargetKind`
- `novex-memory`: `MemoryScope`, `MemoryWritePolicy`
- `novex-eval`: `EvalTargetKind`, `EvalMetricKind`

Each crate can depend on `novex-ai-core` for `FoundationModule` and `FoundationStatus`.

**Step 4: Run tests**

Run: `cargo test --workspace`

Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml crates
git commit -m "feat: add ai foundation crate skeletons"
```

### Task 3: Add Backend AI Foundation Endpoint

**Files:**
- Create: `backend/src/application/ai/mod.rs`
- Create: `backend/src/application/ai/foundation_service.rs`
- Create: `backend/src/interfaces/http/ai/mod.rs`
- Create: `backend/src/interfaces/http/ai/foundation.rs`
- Modify: `backend/src/application/mod.rs`
- Modify: `backend/src/interfaces/http/mod.rs`
- Modify: `backend/Cargo.toml`
- Test: `backend/src/interfaces/http/ai/foundation.rs`

**Step 1: Write failing route tests**

Add tests that:

- call `GET /ai/foundation/summary` with a current user containing `ai:foundation:read`
- assert response `code == "200"` and contains `novex-ai-core`, `novex-model`, and `novex-rag`
- call the service permission path without the permission and assert forbidden behavior

**Step 2: Run test to verify it fails**

Run: `cargo test -p backend ai::foundation`

Expected: FAIL because the route and modules do not exist.

**Step 3: Add minimal implementation**

Implement `FoundationService::summary()` that aggregates metadata from AI crates. Add routes:

```rust
Router::new().route("/ai/foundation/summary", get(summary))
```

Protect it with:

```rust
require_permission(&current_user, "ai:foundation:read")?;
```

**Step 4: Run backend tests**

Run: `cargo test -p backend`

Expected: PASS.

**Step 5: Commit**

```bash
git add backend
git commit -m "feat: expose ai foundation summary endpoint"
```

### Task 4: Seed AI Menus And Permissions

**Files:**
- Create: `backend/migrations/202606050001_seed_ai_foundation_menus.sql`

**Step 1: Write migration**

Seed:

- AI top-level catalog `AI 基座`
- AI child menus for Dashboard, Models, Knowledge, Agents, Tools, Connectors, Plugins, Triggers, Evals, Traces, Templates
- button permissions such as `ai:foundation:read`, `ai:model:list`, `ai:knowledge:list`
- system identity menu placeholders and permissions
- admin role grants for all inserted menu IDs

**Step 2: Review idempotency**

Ensure every `INSERT` uses stable IDs and `ON CONFLICT DO NOTHING`.

**Step 3: Run migration/test command**

Run: `cargo test -p backend`

Expected: PASS. SQLx compile checks should accept the migration file.

**Step 4: Commit**

```bash
git add backend/migrations/202606050001_seed_ai_foundation_menus.sql
git commit -m "feat: seed ai foundation menus"
```

### Task 5: Add Sidecar, App, Template, And Infra Skeletons

**Files:**
- Create: `services/parser-worker/README.md`
- Create: `services/model-runtime/README.md`
- Create: `apps/chat-web/README.md`
- Create: `apps/training-web/README.md`
- Create: `apps/agent-workspace/README.md`
- Create: `templates/README.md`
- Create: `templates/llm-chat/README.md`
- Create: `templates/knowledge-base-chat/README.md`
- Create: `templates/agent-workspace/README.md`
- Create: `templates/training-app/README.md`
- Create: `infra/README.md`

**Step 1: Add boundary docs**

Each README states ownership, allowed dependencies, and M0 non-goals.

**Step 2: Run repository checks**

Run: `find services apps templates infra -maxdepth 2 -type f | sort`

Expected: all skeleton files listed.

**Step 3: Commit**

```bash
git add services apps templates infra
git commit -m "docs: add foundation service and app skeletons"
```

### Task 6: Add Admin AI Placeholder Pages

**Files:**
- Create: `admin/app/(main)/ai/dashboard/page.tsx`
- Create: `admin/app/(main)/ai/models/page.tsx`
- Create: `admin/app/(main)/ai/knowledge/page.tsx`
- Create: `admin/app/(main)/ai/agents/page.tsx`
- Create: `admin/app/(main)/ai/tools/page.tsx`
- Create: `admin/app/(main)/ai/connectors/page.tsx`
- Create: `admin/app/(main)/ai/plugins/page.tsx`
- Create: `admin/app/(main)/ai/triggers/page.tsx`
- Create: `admin/app/(main)/ai/evals/page.tsx`
- Create: `admin/app/(main)/ai/traces/page.tsx`
- Create: `admin/app/(main)/ai/templates/page.tsx`
- Create: `admin/app/(main)/system/identity/providers/page.tsx`
- Create: `admin/app/(main)/system/identity/accounts/page.tsx`
- Create: `admin/app/(main)/system/identity/policies/page.tsx`
- Optional Create: `admin/src/components/ai/foundation-placeholder.tsx`

**Step 1: Write a small reusable placeholder component**

Implement a compact work-focused page component with title, boundary, status, and next milestone. Keep it static and permission-driven by the existing menu route system.

**Step 2: Add pages**

Each page imports the placeholder component and passes module-specific text.

**Step 3: Run frontend checks**

Run: `pnpm typecheck`

Expected: PASS.

Run: `pnpm test`

Expected: PASS.

**Step 4: Commit**

```bash
git add admin
git commit -m "feat: add ai foundation admin placeholders"
```

### Task 7: Final Verification

**Files:**
- No new files.

**Step 1: Run full Rust verification**

Run: `cargo test --workspace`

Expected: PASS.

**Step 2: Run admin verification**

Run: `pnpm typecheck`

Expected: PASS.

Run: `pnpm test`

Expected: PASS.

**Step 3: Check git status**

Run: `git status --short`

Expected: clean after commits.

**Step 4: Report outcome**

Summarize implemented files, commits, and verification output.

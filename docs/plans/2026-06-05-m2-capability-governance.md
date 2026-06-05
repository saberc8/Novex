# M2 Capability Governance Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build M2 registries and dry-run POCs for skills, tools, connectors, plugins, triggers, MCP servers, and tool call audit.

**Architecture:** Keep governance metadata and RBAC-protected HTTP APIs in `backend`. Keep reusable capability vocabulary in the existing M2 crates (`novex-tools`, `novex-connectors`, `novex-plugin`, `novex-trigger`, `novex-mcp`) and avoid real external execution in M2. Admin pages consume summary APIs and dry-run audit APIs; real connector credentials and unsafe tool execution are deferred.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL migrations, Next.js Admin, Vitest.

---

### Task 1: Capability Registry Schema and Seeds

**Files:**
- Create: `backend/migrations/202606050006_create_ai_capability_registry.sql`
- Create: `backend/migrations/202606050007_seed_ai_capability_permissions.sql`

**Step 1: Add schema and seed data**

Create registry tables:

- `ai_skill`
- `ai_tool`
- `ai_connector`
- `ai_plugin`
- `ai_trigger`
- `ai_mcp_server`
- `ai_tool_call_audit`

Seed dry-run POCs:

- GitHub connector/provider metadata.
- Feishu connector metadata and Feishu message tool.
- Media image generation tool.
- RAG search tool.
- Webhook trigger metadata.
- MCP local dry-run server.
- Knowledge/training skills.
- Builtin plugin metadata.

Add permissions:

- `ai:tool:dryRun`
- `ai:tool:audit:list`
- `ai:mcp:list`

**Step 2: Verify**

Run:

```bash
rg "CREATE TABLE IF NOT EXISTS ai_tool|CREATE TABLE IF NOT EXISTS ai_connector|CREATE TABLE IF NOT EXISTS ai_tool_call_audit" backend/migrations/202606050006_create_ai_capability_registry.sql
rg "ai:tool:dryRun|ai:tool:audit:list|ai:mcp:list" backend/migrations/202606050007_seed_ai_capability_permissions.sql
cargo test -p backend-rust --offline
```

**Step 3: Commit**

```bash
git add backend/migrations/202606050006_create_ai_capability_registry.sql backend/migrations/202606050007_seed_ai_capability_permissions.sql
git commit -m "feat: add ai capability registry schema"
```

### Task 2: Backend Capability Summary API

**Files:**
- Create: `backend/src/infrastructure/persistence/ai_capability_repository.rs`
- Create: `backend/src/application/ai/capability_service.rs`
- Create: `backend/src/interfaces/http/ai/capability.rs`
- Modify: `backend/src/infrastructure/persistence/mod.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`

**Step 1: Write failing tests**

Test:

- Summary permissions match seeded permission strings.
- Missing permission returns `Forbidden`.
- Capability route is registered and requires auth.
- Query normalization defaults to enabled POC records.

Run:

```bash
cargo test -p backend-rust capability --offline
```

Expected: fail because capability modules do not exist.

**Step 2: Implement summary query**

Add read-only endpoints:

- `GET /ai/capabilities/summary`
- `GET /ai/capabilities/tools`
- `GET /ai/capabilities/connectors`
- `GET /ai/capabilities/plugins`
- `GET /ai/capabilities/triggers`
- `GET /ai/capabilities/mcp-servers`

Return camelCase DTOs with `id`, `code`, `name`, `kind`, `status`, `riskLevel`, `metadata`, and `createTime` where relevant.

**Step 3: Verify and commit**

```bash
cargo test -p backend-rust capability --offline
cargo test -p backend-rust --offline
git add backend/src/infrastructure/persistence/ai_capability_repository.rs backend/src/application/ai/capability_service.rs backend/src/interfaces/http/ai/capability.rs backend/src/infrastructure/persistence/mod.rs backend/src/application/ai/mod.rs backend/src/interfaces/http/ai/mod.rs
git commit -m "feat: add capability summary api"
```

### Task 3: Dry-Run Tool Audit API

**Files:**
- Modify: `backend/src/application/ai/capability_service.rs`
- Modify: `backend/src/infrastructure/persistence/ai_capability_repository.rs`
- Modify: `backend/src/interfaces/http/ai/capability.rs`

**Step 1: Write failing tests**

Test:

- Blank dry-run tool code is rejected.
- Dry-run response includes `auditId`, `toolCode`, `status`, and `dryRun=true`.
- Audit list route requires `ai:tool:audit:list`.

Run:

```bash
cargo test -p backend-rust tool_dry_run --offline
```

Expected: fail because dry-run API does not exist.

**Step 2: Implement dry-run and audit list**

Add:

- `POST /ai/capabilities/tools/dry-run`
- `GET /ai/capabilities/tools/audits`

Dry-run never performs external calls. It validates registered tool code, writes `ai_tool_call_audit`, and returns a deterministic payload.

**Step 3: Verify and commit**

```bash
cargo test -p backend-rust tool_dry_run --offline
cargo test -p backend-rust --offline
git add backend/src/application/ai/capability_service.rs backend/src/infrastructure/persistence/ai_capability_repository.rs backend/src/interfaces/http/ai/capability.rs
git commit -m "feat: add dry run tool audit api"
```

### Task 4: Admin Capability Pages

**Files:**
- Create: `admin/src/types/ai-capability.ts`
- Create: `admin/src/api/ai/capability.ts`
- Create: `admin/src/api/ai/capability.test.ts`
- Create: `admin/src/components/ai/capability-registry.tsx`
- Modify: `admin/app/(main)/ai/tools/page.tsx`
- Modify: `admin/app/(main)/ai/connectors/page.tsx`
- Modify: `admin/app/(main)/ai/plugins/page.tsx`
- Modify: `admin/app/(main)/ai/triggers/page.tsx`

**Step 1: Write failing tests**

Test API wrappers for:

- `getCapabilitySummary`
- `listTools`
- `dryRunTool`
- `listToolAudits`

Run:

```bash
pnpm vitest run src/api/ai/capability.test.ts
```

Expected: fail because wrappers do not exist.

**Step 2: Implement Admin summary components**

Replace placeholders with registry views that show seeded POCs and a dry-run button for tools.

**Step 3: Verify and commit**

```bash
pnpm typecheck
pnpm vitest run src/api/ai/capability.test.ts
pnpm lint
git add admin/src/types/ai-capability.ts admin/src/api/ai/capability.ts admin/src/api/ai/capability.test.ts admin/src/components/ai/capability-registry.tsx 'admin/app/(main)/ai/tools/page.tsx' 'admin/app/(main)/ai/connectors/page.tsx' 'admin/app/(main)/ai/plugins/page.tsx' 'admin/app/(main)/ai/triggers/page.tsx'
git commit -m "feat: add capability registry admin pages"
```

### Task 5: M2 Verification

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd admin && pnpm typecheck && pnpm vitest run && pnpm lint && pnpm build
```

Restart backend with `DB_AUTO_MIGRATE=true`, then smoke:

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
curl http://localhost:4399/ai/tools
```

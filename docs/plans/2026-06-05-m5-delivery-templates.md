# M5 Delivery Templates Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the M5 customer delivery template control plane so Novex can list default app templates, generate a customer initialization package, and document the deployment flow.

**Architecture:** Template package manifests live under `templates/*/template.json` as the delivery artifact source of truth. The Rust backend embeds and validates those manifests, exposes read/init APIs through the existing AI/RBAC HTTP layer, and keeps initialization package generation deterministic for POC. The admin page consumes those APIs as a compact customer initialization wizard; no full marketplace, tenant provisioning, or persistent package history is introduced in M5.

**Tech Stack:** Rust, Axum, Serde JSON, SQL migration for RBAC permissions, Next.js Admin, Vitest.

---

### Task 1: Template Manifest Artifacts

**Files:**
- Create: `templates/llm-chat/template.json`
- Create: `templates/knowledge-base-chat/template.json`
- Create: `templates/agent-workspace/template.json`
- Create: `templates/training-app/template.json`
- Modify: `templates/README.md`
- Modify: `templates/*/README.md`

**Step 1: Add static manifests**

Each manifest must include:

- `code`, `name`, `category`, `description`, `frontendEntry`, `sort`, `status`
- `branding` defaults
- `roles` and `menus`
- `prompts`, `skills`, `connectors`, `plugins`, `triggers`
- `evalSets`
- `deploymentChecklist`

The four required codes are `llm_chat`, `knowledge_base_chat`, `agent_workspace`, and `training_app`.

**Step 2: Verify manifests**

```bash
rg '"code": "(llm_chat|knowledge_base_chat|agent_workspace|training_app)"' templates/*/template.json
rg '"roles"|"menus"|"branding"|"evalSets"|"deploymentChecklist"' templates/*/template.json
```

Expected: all four manifests include the required sections.

**Step 3: Commit**

```bash
git add templates docs/plans/2026-06-05-m5-delivery-templates.md
git commit -m "docs: add delivery template manifests"
```

### Task 2: Backend Template Package API

**Files:**
- Create: `backend/src/application/ai/template_service.rs`
- Create: `backend/src/interfaces/http/ai/template.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`
- Create: `backend/migrations/202606050012_seed_ai_template_permissions.sql`

**Step 1: Write failing tests**

Add tests for:

- `delivery_templates_include_all_m5_defaults`
- `delivery_template_manifest_requires_roles_menus_branding_and_eval_sets`
- `customer_package_generation_merges_customer_branding`
- HTTP handler rejects missing `ai:template:init`
- route registration requires auth
- permission seed contains `ai:template:list` and `ai:template:init`

Run:

```bash
cargo test -p backend delivery_template --offline
```

Expected: fail because the template service and routes do not exist.

**Step 2: Implement service and routes**

Expose:

- `GET /ai/templates`
- `GET /ai/templates/:code`
- `POST /ai/templates/packages`

Behavior:

- Load the four embedded template manifests from `templates/*/template.json`.
- Support list filters `category` and `status`.
- Generate a deterministic customer package response with `packageId`, `template`, `tenantConfig`, `branding`, `roles`, `menus`, `skills`, `connectors`, `plugins`, `triggers`, `evalSets`, `deploymentChecklist`, and `deploymentSteps`.
- Normalize init command fields and reject blank `templateCode`, `customerName`, or `appName`.
- Require `ai:template:list` for list/detail and `ai:template:init` for package generation.

**Step 3: Verify and commit**

```bash
cargo test -p backend delivery_template --offline
cargo test -p backend --offline
cargo fmt -- --check
git add backend/src/application/ai/template_service.rs backend/src/interfaces/http/ai/template.rs backend/src/application/ai/mod.rs backend/src/interfaces/http/ai/mod.rs backend/migrations/202606050012_seed_ai_template_permissions.sql
git commit -m "feat: add delivery template api"
```

### Task 3: Admin Template Initialization Wizard

**Files:**
- Create: `admin/src/types/ai-template.ts`
- Create: `admin/src/api/ai/template.ts`
- Create: `admin/src/api/ai/template.test.ts`
- Modify: `admin/app/(main)/ai/templates/page.tsx`

**Step 1: Write failing tests**

Test wrappers for:

- `listDeliveryTemplates`
- `getDeliveryTemplate`
- `generateCustomerPackage`

Run:

```bash
pnpm vitest run src/api/ai/template.test.ts
```

Expected: fail because wrappers do not exist.

**Step 2: Implement admin page**

Replace the placeholder with:

- Template list/cards for the four default templates.
- Customer initialization form with template selector, customer name, app name, industry, brand name, primary color, and public URL.
- Init button guarded by `ai:template:init`.
- Generated package preview showing roles, menus, connectors/plugins/triggers, eval sets, and deployment checklist.

**Step 3: Verify and commit**

```bash
pnpm typecheck
pnpm vitest run src/api/ai/template.test.ts
pnpm lint
git add admin/src/types/ai-template.ts admin/src/api/ai/template.ts admin/src/api/ai/template.test.ts 'admin/app/(main)/ai/templates/page.tsx'
git commit -m "feat: add template initialization wizard"
```

### Task 4: Delivery Manual

**Files:**
- Create: `docs/delivery/novex-customer-delivery.md`
- Modify: `docs/ARCHITECTURE.md` only if a small pointer is useful.

**Step 1: Write the manual**

Document:

- The four M5 default templates.
- Customer package contents.
- Initialization flow from tenant config to eval smoke test.
- Required environment/config checkpoints.
- How template manifests map to roles, menus, skills, connectors, plugins, triggers, eval sets, and frontend entries.
- POC limitations: no full marketplace, no live tenant creation, no connector OAuth binding automation.

**Step 2: Verify and commit**

```bash
rg "LLM Chat|Knowledge Base Chat|Agent Workspace|Training App|eval smoke|connector" docs/delivery/novex-customer-delivery.md
git add docs/delivery/novex-customer-delivery.md
git commit -m "docs: add customer delivery manual"
```

### Task 5: M5 Verification and Smoke

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
curl http://localhost:4399/ai/templates
curl -H "Authorization: Bearer $TOKEN" http://localhost:4398/ai/templates
curl -H "Authorization: Bearer $TOKEN" http://localhost:4398/ai/templates/training_app
curl -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{"templateCode":"training_app","customerName":"Acme","appName":"Acme Training","industry":"training","brandName":"Acme Academy","primaryColor":"#2563eb","publicUrl":"https://training.example.com"}' http://localhost:4398/ai/templates/packages
```

Expected: template list contains four templates and package generation returns customer branding, roles, menus, connector/plugin/trigger config, eval sets, and deployment steps.

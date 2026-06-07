# Novex Customer Delivery Manual

This manual describes the M5 customer delivery flow for turning Novex foundation
capabilities into a repeatable customer package.

## Default Templates

Novex ships four default templates:

- LLM Chat: pure model chat for general assistants, drafting, brainstorming, and simple support.
- Knowledge Base Chat: RAG chat with citations for policies, documentation, FAQ, and product knowledge.
- Agent Workspace: tool-using workspace with approvals, traces, connector actions, and Agent eval coverage.
- Training App: employee training POC with knowledge Q&A, quiz generation, learning records, Feishu reminders, and eval smoke checks.

Template manifests live in:

- `templates/llm-chat/template.json`
- `templates/knowledge-base-chat/template.json`
- `templates/agent-workspace/template.json`
- `templates/training-app/template.json`

The backend embeds these manifests and exposes them through `GET /ai/templates`.

## Customer Package Contents

A generated customer package contains:

- Tenant config: customer name, app name, industry, template code, and frontend entry.
- Branding: brand name, logo text, primary color, and public URL.
- Frontend config: app id, frontend entry, entry URL, default page, navigation, and roles allowed to access app pages.
- Provisioning plan: machine-readable steps for tenant, roles, menus, frontend config, capabilities, eval sets, and smoke checks.
- Roles and menus: default app roles, menu entries, and permission codes.
- Prompts and skills: baseline prompt contracts and skill entries.
- Connectors: external resource configuration such as Web Import, GitHub Repository, or Feishu Message.
- Plugins: built-in adapter packages that group tools, connectors, triggers, UI config, and eval cases.
- Triggers: webhook or schedule entry points routed to jobs or Agent runs.
- Eval sets: regression or smoke suites that must pass before pilot.
- Deployment checklist and deployment steps.

The M5 API can apply the tenant, role, menu, frontend config, capability registry, and eval set selection steps through `POST /ai/templates/packages/apply`. The apply endpoint upserts `sys_tenant`, creates tenant-scoped template roles, creates hidden app permission menu nodes, binds roles through `sys_tenant_role` and `sys_role_menu`, stores the customer frontend branding/navigation/default route snapshot, enables template skills/connectors/plugins/triggers in the tenant capability registry, installs built-in template plugins, copies selected template eval datasets/cases into the customer tenant, persists an `ai_customer_package` snapshot, and returns `appliedSteps` plus `pendingOperatorSteps`. Operators can trigger template smoke planning or execution from the Admin customer initialization page or through `POST /ai/templates/smoke/runs`, which records run and check-level results. Connector credentials, model credentials, frontend deployment, eval execution, smoke execution during apply, and production deployment remain operator-controlled because those surfaces require environment-specific secrets, explicit run triggers, or deployment targets.

## Initialization Flow

1. Select the default template.
2. Apply the package tenant/role/menu/frontend config/capability registry/eval selection steps or select an existing tenant.
3. Review the applied role/menu bindings for the package.
4. Deploy the selected frontend app to the public URL.
5. Configure model routes, model credentials, embedding, and rerank settings.
6. Configure identity providers separately from connector credentials.
7. Bind connector credentials with explicit scope.
8. Import knowledge datasets if the template uses RAG.
9. Review enabled skills, tools, plugins, and triggers.
10. Run the selected template eval set through the eval runtime API.
11. Run template smoke checks through `POST /ai/templates/smoke/runs`.
12. Release the app for customer pilot.

The Training App template should run `training_regression` before pilot. Knowledge Base Chat should run `knowledge_base_regression`. Agent Workspace should run `agent_workspace_regression`. LLM Chat should run `llm_chat_smoke`.

## Publish/Share Flow

Published customer apps use Public Link records from the Admin integration entry
surface. Operators create them under `ai:integration:create`, choose the target
template app, set the target path, and grant only the app permission required by
that route. For the chat templates, the publish/share route is
`/share/[token]` in `apps/chat-web`.

Public Link setup checklist:

- Select `llm_chat` with path `/chat` and scope `app:chat:use` for pure model chat.
- Select `knowledge_base_chat` with path `/knowledge` and scope `app:knowledge:ask` for RAG chat.
- Set QPS and quota limits before customer pilot.
- Set an expiry date for temporary pilots and revoke expired links from Admin.
- Verify the generated public URL resolves to `/share/[token]` and displays the runtime context without exposing the raw token.

The share page is the public or semi-public entry point. The backend resolves the
token, enforces the Public Link tenant, permission scope, QPS, quota, expiry, and
usage audit contract, then returns sanitized runtime context to the frontend.
Business execution must still use scoped Novex APIs; raw secrets and connector
credentials are never embedded in the share page.

## Environment Checkpoints

Before delivery:

- Backend has `DB_AUTO_MIGRATE=true` for first startup in the target environment.
- `AUTH_JWT_SECRET` is set to a non-placeholder secret.
- The selected model route is reachable from backend runtime.
- Object storage or local storage is configured for files and media.
- External connector credentials are stored through the controlled secret path, not in template JSON.
- Public URL, CORS origin, and frontend deployment target match the customer domain.

## Manifest Mapping

Manifest fields map to delivery surfaces:

- `branding` maps to customer frontend configuration.
- `frontendConfig` maps to the persisted `apps/*` entry URL, navigation, default page, branding, and allowed frontend roles.
- `provisioningPlan` maps package contents into ordered steps with target surface, operation, and payload.
- `appliedSteps` confirms the tenant, role, menu, frontend config, capability registry, plugin installation, eval dataset/case selection, and package snapshot records written by the apply endpoint.
- `pendingOperatorSteps` lists smoke work because package apply does not automatically execute checks; operators can run it through the template smoke API. Deployment is recorded as frontend config but still requires an environment runner.
- `roles` maps to system role creation and role permission grants.
- `menus` maps to app navigation and resource permission checks.
- `prompts` maps to prompt registry entries or app-level prompt config.
- `skills` maps to enabled skills for the selected template.
- `connectors` maps to connector registry records and credential binding tasks.
- `plugins` maps to built-in plugin registry/version/capability/installation records and permission review.
- `triggers` maps to trigger routing and schedule/webhook setup metadata.
- `evalSets` maps to eval dataset/case selection; regression execution is triggered through the eval runtime API after apply.
- `frontendEntry` maps to the customer app template under `apps/*`.

## POC Limits

M5 intentionally does not implement:

- Full plugin marketplace.
- Connector OAuth automation.
- Model credential storage from the template API.
- One-click frontend publishing from the template API.
- Automatic eval/smoke execution during package apply.
- One-click production deploy.

Those are later delivery platform features. The M5 boundary is a repeatable package plan, tenant role/menu/package snapshot apply, admin initialization wizard, default template artifacts, and smoke-testable backend APIs.

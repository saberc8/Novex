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
- Roles and menus: default app roles, menu entries, and permission codes.
- Prompts and skills: baseline prompt contracts and skill entries.
- Connectors: external resource configuration such as Web Import, GitHub Repository, or Feishu Message.
- Plugins: built-in adapter packages that group tools, connectors, triggers, UI config, and eval cases.
- Triggers: webhook or schedule entry points routed to jobs or Agent runs.
- Eval sets: regression or smoke suites that must pass before pilot.
- Deployment checklist and deployment steps.

The M5 API does not create tenants, bind credentials, or install plugins automatically. It returns a package plan that an operator can apply through the existing RBAC, knowledge, connector, tool, eval, and deployment surfaces.

## Initialization Flow

1. Select the default template.
2. Create or select the tenant.
3. Initialize roles and menus from the package.
4. Apply branding and frontend public URL.
5. Configure model routes, model credentials, embedding, and rerank settings.
6. Configure identity providers separately from connector credentials.
7. Bind connector credentials with explicit scope.
8. Import knowledge datasets if the template uses RAG.
9. Enable skills, tools, plugins, and triggers.
10. Run the template eval smoke set.
11. Release the app for customer pilot.

The Training App template should run `training_regression` before pilot. Knowledge Base Chat should run `knowledge_base_regression`. Agent Workspace should run `agent_workspace_regression`. LLM Chat should run `llm_chat_smoke`.

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
- `roles` maps to system role creation and role permission grants.
- `menus` maps to app navigation and resource permission checks.
- `prompts` maps to prompt registry entries or app-level prompt config.
- `skills` maps to enabled skills for the selected template.
- `connectors` maps to connector registry records and credential binding tasks.
- `plugins` maps to built-in plugin enablement and permission review.
- `triggers` maps to trigger routing and schedule/webhook setup.
- `evalSets` maps to eval dataset selection and regression runs.
- `frontendEntry` maps to the customer app template under `apps/*`.

## POC Limits

M5 intentionally does not implement:

- Full plugin marketplace.
- Live tenant creation from the template API.
- Connector OAuth automation.
- Model credential storage from the template API.
- One-click production deploy.
- Persistent customer package history.

Those are later delivery platform features. The M5 boundary is a repeatable package plan, admin initialization wizard, default template artifacts, and smoke-testable backend APIs.

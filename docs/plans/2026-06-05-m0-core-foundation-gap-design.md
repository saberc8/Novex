# M0 Core Foundation Gap Design

Date: 2026-06-05

## Context

`docs/ARCHITECTURE.md` defines a broader foundation than the current M0-M5 POC. The current implementation is runnable, but several base contracts are still missing: tenant control plane, resource ACL, secret references, quota/usage/rate-limit tables, persistent model registry tables, and model route usage in RAG traces.

This slice closes the highest-impact M0 gaps without replacing the deterministic POC runtime.

## Scope

1. Add platform control-plane tables:
   - `sys_tenant`, `sys_tenant_user`, `sys_tenant_role`
   - `sys_member_group`, `sys_member_group_user`
   - `sys_resource_permission`
   - `sys_quota_policy`, `sys_usage_meter`, `sys_rate_limit_policy`
   - `sys_identity_provider`, `sys_external_account`, `sys_oauth_state`
   - `sys_secret`

2. Add model registry tables:
   - `ai_model_provider`, `ai_model_deployment`, `ai_model_profile`
   - `ai_model_credential`, `ai_model_route`
   - `ai_model_health_check`, `ai_model_usage`

3. Seed safe defaults:
   - Default platform tenant and admin membership.
   - Provider/profile/route metadata for the environment-backed DeepSeek, DashScope embedding, DashScope reranker, and Right Code Draw routes.
   - No raw API keys in migrations or code.

4. Expose read-only model registry summaries through the existing model API permission.

5. Replace hard-coded RAG trace route names with `novex-model` runtime route IDs when the environment provides them, falling back to local route names when not configured.

## Non-Goals

- No encrypted secret write UI yet.
- No tenant switching UI yet.
- No full ACL enforcement beyond existing RBAC and existing `tenant_id = 1` POC behavior.
- No Milvus, parser worker execution, PDF/Office parsing, or external embedding writes yet.
- No real tool/connector/plugin/trigger execution.

## Design

### Control Plane Tables

The new system tables use the existing style: explicit `BIGINT` primary keys, `tenant_id` where applicable, status fields, JSONB policy payloads, and audit columns. This preserves future flexibility while keeping the migration safe for the current single-tenant POC.

`sys_secret` stores `ciphertext` and `masked_value`, but this slice only defines the contract. Runtime API keys remain environment variables until a separate credential management slice adds encryption and rotation.

### Model Registry Tables

The persistent registry captures architecture-level metadata:

- Provider: vendor identity and protocol.
- Deployment: endpoint, network zone, timeout and concurrency limits.
- Profile: model kind, model name, capabilities, cost, embedding/rerank specs.
- Credential: secret reference and masked value.
- Route: purpose-level route binding.
- Health/Usage: normalized operational records.

The existing env-backed runtime config remains the active runtime source for this slice. Registry rows make the control plane visible and prepare for tenant/app/skill/task routing.

### API

Add `GET /ai/models/registry` under `ai:model:list`. It returns provider/deployment/profile/route summaries from the new tables and a count summary. It does not return raw secrets.

### RAG Route Trace

RAG still runs deterministic local retrieval and extractive answer generation in this slice. Trace rows change from fixed `local-keyword` / `none` / `local-extractive` to:

- env runtime route IDs when the relevant runtime target is configured;
- local fallback names when not configured.

This makes trace data prove the model route resolution boundary without forcing semantic embedding/LLM calls in the same patch.

## Safety

- Migrations are additive and use `IF NOT EXISTS` / `ON CONFLICT DO NOTHING`.
- No raw API keys are inserted into the database.
- Existing APIs keep their behavior.
- Tests assert required table names, key fields, no fake raw secret leaks, route registration, and RAG fallback behavior.

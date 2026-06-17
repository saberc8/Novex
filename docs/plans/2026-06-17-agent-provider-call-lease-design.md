# Agent Provider Call Lease Design

## Context

Novex already has Codex-style model-loop cancellation checkpoints, a process-local runtime registry, a persistent run-status watcher around provider futures, provider retry/fallback traces, and Responses-compatible compaction transport parsing.

The remaining runtime gap is that an in-flight provider call is still not represented as a durable control-plane resource. If a worker process is killed, a provider call stalls, or a later provider-native cancel endpoint is added, Novex has no table that tells operators which provider call belongs to which tenant/run/route and what lifecycle state it is in.

Codex keeps turn/task/provider stream work tied to task lifecycle and rollout attempts. Novex needs the enterprise-control-plane equivalent: a durable provider-call lease row.

## Decision

Add a provider-call lease boundary inside `ModelRuntimeService`.

The first slice records chat/model provider calls because those are the calls that serve chat flow, POC agent loop, customer service, enterprise knowledge-base answers, NotebookLM-style flows, Guardian review, and context compaction. Embedding/rerank/media calls can reuse the same table in later slices.

## Data Model

Create `ai_model_provider_call_lease` with:

- tenant/run/route identity: `tenant_id`, nullable `run_id`, `route_code`, `route_purpose`, provider type, model;
- local ownership: `lease_owner`, `lease_expires_at`, `heartbeat_at`;
- lifecycle status: `running`, `succeeded`, `failed`, `cancelled`, `expired`;
- request kind and source: `request_kind`, `source`, `attempt_kind`;
- observability: `latency_ms`, token counts, cost cents, `error_kind`, `http_status`, `error_message`;
- structured metadata: request and response JSON snapshots.

The table is intentionally independent of provider-native cancel APIs. It is the durable lifecycle surface provider-native cancel can later target.

## Runtime Flow

1. `ModelChatCommand` gains a serde-skipped `provider_call_context`.
2. Agent model-loop sampling sets `run_id`, source `agent.model_loop`, route purpose `code_agent`, and attempt kind.
3. Agent context compaction sets `run_id`, source `agent.context_compaction`, route purpose `code_agent`, and request kind `compaction`.
4. `chat_completion_for_source`, `chat_completion_with_usage`, and `chat_completion_for_purpose` pass through the tenant-bound lease wrapper.
5. The wrapper inserts a `running` lease before the provider await and completes it as `succeeded`, `failed`, or `cancelled`.
6. `ModelChatResp` carries optional `provider_call_lease_id`; `model_inference` events include it when present.

## Error Handling

Lease insertion/update failures are ordinary persistence errors. This keeps enterprise observability honest: if the control plane cannot record provider lifecycle state, the model call should not silently bypass it.

The existing static/env-only `ModelRuntimeService::chat_completion` helper remains a compatibility escape hatch and does not record leases because it has no tenant DB context.

## Scope Boundaries

This slice does not implement:

- provider-native cancellation endpoints;
- lease heartbeat refresh from a long-running streaming worker;
- embedding/rerank/media leases;
- active lease list/clear HTTP controls;
- replacing the persistent run-status polling watcher.

## Acceptance

- Migration defines `ai_model_provider_call_lease` with tenant/run/route/status/lease indexes.
- Source-contract tests prove tenant-bound model calls wrap provider awaits with lease begin/complete.
- Unit tests prove lease records map route, metadata, success usage, failure class, and cancelled status.
- Agent model-loop and context compaction commands carry local provider-call context without leaking it into provider request metadata.
- `model_inference` event payload includes `providerCallLeaseId` when a response has one.

# Agent Provider Embedding/Rerank Lease Design

## Goal

Extend the durable provider-call lease control plane beyond chat/model calls into the RAG retrieval path.

Enterprise knowledge-base and NotebookLM-style workflows depend heavily on embedding and rerank provider calls. Before this slice, those calls used raw static provider helpers, so operator lease listing, stale-expire recovery, heartbeat fields, and model ops evidence covered Agent chat/model calls but not RAG retrieval provider work.

## Scope

This slice adds tenant-bound leases for RAG provider calls:

- `ModelRuntimeService::embed_texts_for_source`
  - Wraps embedding provider calls in `ai_model_provider_call_lease`.
  - Uses `request_kind = embedding`.
  - Stores route/provider/model metadata and input counts, not input text or API keys.
- `ModelRuntimeService::rerank_documents_for_source`
  - Wraps rerank provider calls in `ai_model_provider_call_lease`.
  - Uses `request_kind = rerank`.
  - Stores query/document counts, not query or document text.
- Shared helper:
  - Creates a lease row.
  - Starts the existing heartbeat refresher.
  - Completes the lease as `succeeded` or `failed`.
- Knowledge service:
  - Runtime chunk embedding, query embedding, and rerank now use tenant-bound service methods.

## Non-Goals

- Media image generation leases.
- Embedding/rerank provider-native cancellation endpoints.
- Token/cost estimation for embedding/rerank payloads.
- New HTTP endpoints; existing provider-call lease list/expire controls already surface `request_kind`.

## Acceptance

- Tests prove embedding lease records map tenant, route, purpose, request kind, source, counts, and avoid secret leakage.
- Source-contract tests prove embedding/rerank wrappers call the shared lease begin/complete path.
- Source-contract tests prove knowledge retrieval no longer calls raw static embedding/rerank provider helpers in production paths.
- Existing provider-call lease, runtime embedding, and rerank tests pass.

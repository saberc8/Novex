# Agent Model Compaction Design

## Context

Novex already has a deterministic context compaction slice: after enough tool observations, the model loop writes a `ContextCompaction` event and continues with a shorter prompt. Codex goes further. It can route compaction through model-backed compact tasks, record compaction lifecycle evidence, and install replacement history while preserving a fallback path.

The current Novex gap is not the presence of compaction. The gap is the adapter boundary: the runtime can only install its own deterministic summary, and the backend cannot use the configured model route to produce a higher-quality compacted summary for long enterprise runs.

## Goal

Add a model-assisted compaction adapter for `runtimeMode=model_loop`. The adapter should use the existing configured `code_agent` model route to rewrite deterministic compaction candidates, record compaction strategy/status metadata, and fall back to deterministic summary when the model call fails.

## Codex References

- `codex-rs/core/src/compact.rs`: inline compact lifecycle and installed `ContextCompaction` item.
- `codex-rs/core/src/compact_remote.rs`: remote compact endpoint, checkpoint metadata, and output rewrite before compacting.
- `codex-rs/core/src/state/auto_compact_window.rs`: window accounting and compact boundaries.

## Design

### Runtime Contract

`novex-agent-runtime` should expose two distinct operations:

- Build a deterministic compaction candidate from items since the last compaction.
- Install a chosen summary as the next `ContextCompaction` item.

This keeps the runtime deterministic and testable while allowing backend adapters to choose the summary source. Existing `compact_context()` remains as a deterministic convenience path.

### Backend Adapter

When `AgentService` detects `runtime_state.should_compact_context()`:

1. Build the deterministic candidate summary from runtime state.
2. Call `ModelRuntimeService::chat_completion_for_purpose(ModelRoutePurpose::CodeAgent, ...)` with a compacting prompt.
3. If the call succeeds, install the model-generated summary.
4. If the call fails, install the deterministic summary.
5. If the run is cancelled while waiting for the compaction model call, finish as cancelled.
6. Persist one `ContextCompaction` event with `compactionStrategy`, `compactionStatus`, window counts, and optional model/error metadata.

The compaction model call uses the same model route registry, retry/fallback/circuit-breaker behavior, provider attempt metadata, and cost estimation as normal model-loop sampling.

### Trace and Eval

Trace conversion already treats `context_compaction` payloads as `TraceEventKind::ContextCompaction`. Eval should enrich trace-derived candidates with compaction strategy evidence:

- `modelCompactionCount`
- `compactionFallbackCount`
- `compactionStatus`

These tags let enterprise knowledge-base, customer-service, and NotebookLM-style eval suites segment long-context regressions by compaction behavior.

## Error Handling

Model compaction failure must not fail the agent run. It records fallback metadata and continues with deterministic summary. Cancellation remains terminal because it reflects operator intent, not compaction quality.

## Non-Goals

- Dedicated compaction model route or new `ModelRoutePurpose`.
- Remote compact endpoint parity.
- New database tables.
- Streaming compaction lifecycle events.
- Token-window accurate truncation.

## Validation

- Runtime tests prove a caller can install model-generated summaries.
- Backend tests prove the model-loop source calls model compaction, records strategy/status, and falls back.
- Eval tests prove trace-derived candidates expose compaction strategy tags.
- Existing model-loop, runtime-span, and workspace tests remain green.

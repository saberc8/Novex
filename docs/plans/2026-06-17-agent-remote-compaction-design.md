# Agent Remote Compaction Design

## Goal

Close the next Codex context-window gap by adding a remote-compaction endpoint contract to Novex model-loop compaction. This is an adapter port of Codex `compact_remote_v2`: Novex should expose the same request/checkpoint shape and trace evidence even though it still routes the actual summarization through the configured `runtime.llm.code_agent` model route in this slice.

## Codex Reference

Codex remote compaction v2 does four things that Novex does not yet model:

- adds an explicit compaction trigger item to the model request,
- trims retained prompt history to a bounded retained-message budget,
- records the compaction implementation and attempt/checkpoint metadata,
- installs replacement history and records the installed checkpoint.

Novex already has deterministic and model-assisted compaction. The missing layer is the remote endpoint boundary: a stable request object that says what history was compacted, what was retained for the next window, what trigger/reason/phase caused the compact, and which implementation was used.

## Options

### Option A: Add a new provider API now

This would add a dedicated compact endpoint to `ModelRuntimeService` and provider adapters immediately.

Trade-off: closest to Codex transport behavior, but too wide for this slice because Novex providers are OpenAI-compatible chat oriented and the current model service does not expose response-stream compaction.

### Option B: Add only trace labels around existing model compaction

This would add `compactionImplementation = remote` without changing runtime contracts.

Trade-off: cheap, but dishonest. It would look like remote parity in traces while the runtime still cannot produce request/checkpoint evidence.

### Option C: Add the remote endpoint contract and keep transport adapted

Selected. `novex-agent-runtime` builds an `AgentRemoteCompactionRequest` from current runtime items. Backend includes that request in the compacting prompt and persists the serialized request in the `ContextCompaction` event. Eval tags remote implementation evidence. The provider transport remains the existing configured `CodeAgent` route until a later slice adds a dedicated provider method.

## Runtime Contract

Add remote compaction vocabulary to `crates/novex-agent-runtime`:

- `AgentRemoteCompactionImplementation::ResponsesCompactionV2`
- `AgentCompactionTrigger::Auto`
- `AgentCompactionReason::ObservationThreshold`
- `AgentCompactionPhase::ModelLoopFollowUp`
- `AgentRemoteCompactionRequest`

`AgentRuntimeState::remote_compaction_request(tool_codes)` returns a request only when `should_compact_context()` is true. It includes:

- next `window_id`,
- full `input_history` since the last compaction,
- bounded `retained_history`,
- `compacted_item_count`,
- `retained_item_count`,
- visible `tool_codes`.

The first retained-history policy is conservative and deterministic: retain user messages and previous compaction summaries; drop raw tool calls, observations, assistant text, and reasoning from retained history because the generated compaction summary should replace them.

## Backend Adapter

`AgentService` keeps the existing model-assisted compaction flow, but changes the prompt and trace payload:

1. Build deterministic candidate summary.
2. Build `AgentRemoteCompactionRequest` from runtime state.
3. Prompt the configured `runtime.llm.code_agent` route with explicit remote endpoint metadata.
4. Install the model summary or deterministic fallback as before.
5. Persist one `ContextCompaction` event with:
   - `compactionImplementation = responses_compaction_v2`
   - `remoteCompaction = <serialized request>`
   - existing strategy/status/model/error metadata.

This keeps current POC behavior running on configured models while preparing a future `ModelRuntimeService::compact_conversation` provider boundary.

## Trace And Eval

Trace already preserves compaction payloads. `novex-eval` should read:

- `compactionImplementation`
- `remoteCompaction`

and add:

- `remoteCompactionCount`
- `compactionImplementation`

These tags let long-context knowledge-base, customer-service, and NotebookLM eval suites segment failures by compaction implementation.

## Non-Goals

- No new model route purpose.
- No provider-native `/responses/compact` or streaming response parsing.
- No new database table.
- No token-accurate truncation.
- No background compaction worker.

## Acceptance

- Runtime tests prove remote compaction requests expose input history, retained history, counts, implementation, trigger, reason, phase, and tool codes.
- Backend tests prove model-loop compaction prompts include remote endpoint metadata and persisted events include remote request payloads.
- Eval tests prove remote compaction evidence is tagged.
- Migration matrix records remote compaction endpoint contract progress while leaving provider-native remote transport as next.
- Feature and merged `main` both pass `cargo fmt -- --check` and `cargo test --workspace --offline`.

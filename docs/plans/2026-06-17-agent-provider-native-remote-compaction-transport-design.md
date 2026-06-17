# Agent Provider-Native Remote Compaction Transport Design

## Goal

Move Novex Agent compaction one step closer to Codex `compact_remote_v2` by carrying an explicit compaction request envelope through the model runtime layer, not only through the AgentService prompt.

## Codex Reference

Codex remote compaction v2 sends compaction requests with structured turn metadata:

- `request_kind = compaction`
- compaction trigger, reason, implementation, phase, and strategy
- window identity and request metadata
- provider request tracing around the compaction attempt

Novex already records `AgentRemoteCompactionRequest` in Agent events. The missing slice is the model transport boundary: `ModelChatCommand` has no way to say, "this provider call is a compaction request." That keeps compaction indistinguishable from ordinary chat inside model payload construction and future provider adapters.

## Selected Approach

Add an opt-in model request metadata envelope to `ModelChatCommand`.

The envelope is intentionally provider-safe:

- Ordinary chat calls do not include metadata.
- Agent compaction calls set `requestKind = compaction`.
- Provider payloads include a `metadata` object only for providers that can reasonably accept OpenAI-compatible metadata (`openai-compatible`, `azure-openai`, and `local-runtime`).
- Providers without that support still receive the existing prompt-adapter request, while Agent events record the transport envelope for trace/eval continuity.

This is an adapter port of Codex metadata transport. It does not claim full `/responses/compact` parity yet.

## Model Runtime Contract

Add backend model-service types:

- `ModelChatRequestMetadata`
- `ModelChatRequestKind`
- `ModelChatCompactionMetadata`

`ModelChatCommand` gains:

- `request_metadata: Option<ModelChatRequestMetadata>`

`model_chat_request_payload(route, command)` serializes the metadata as:

```json
{
  "metadata": {
    "request_kind": "compaction",
    "compaction": {
      "implementation": "responses_compaction_v2",
      "trigger": "auto",
      "reason": "observation_threshold",
      "phase": "model_loop_follow_up",
      "strategy": "memento",
      "window_id": "1",
      "input_history_count": "2",
      "retained_history_count": "1"
    }
  }
}
```

The values are strings to stay compatible with OpenAI-style metadata constraints.

## AgentService Adapter

When `model_loop_context_compaction_outcome(...)` receives an `AgentRemoteCompactionRequest`, it builds `ModelChatRequestMetadata::remote_compaction(...)` and passes it to `ModelChatCommand`.

The compaction event payload additionally records:

- `modelRequestMetadata`
- `compactionTransport = provider_metadata`

When no remote request exists, the call remains a prompt adapter without transport metadata.

## Non-Goals

- No dedicated `ModelRuntimeService::compact_conversation` method yet.
- No streaming compaction output parser.
- No token-accurate retained-message budget.
- No provider-specific `/responses/compact` endpoint routing.
- No change to ordinary chat, RAG, Guardian, eval judge, or media calls.

## Acceptance

- Model-service tests prove ordinary chat payloads omit metadata.
- Model-service tests prove compaction metadata serializes into provider payloads for OpenAI-compatible routes.
- Model-service tests prove unsupported providers do not receive provider metadata fields.
- AgentService tests prove the compaction adapter passes model request metadata and records it in event payloads.
- Migration matrix records provider-native remote compaction transport metadata as implemented while leaving dedicated endpoint/streaming transport as next.

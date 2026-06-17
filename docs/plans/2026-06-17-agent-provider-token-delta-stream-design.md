# Agent Provider Token Delta Stream Design

## Goal

Move the CodeAgent model loop one step closer to Codex-style live runs by turning provider SSE token deltas into durable `ai_run_event` records that can be replayed by the existing SSE/WebSocket run-event transports.

## Current State

The runtime already has a durable run-event stream, replay cursors, authenticated WebSocket transport, browser WebSocket tickets, provider-call leases, inference trace spans, cancellation checkpoints, and turn-item replay. The remaining gap is that provider output is still treated as a completed `ModelChatResp` before the agent event stream sees any text. `ModelChatResp` also does not preserve provider delta evidence.

## Selected Approach

Add a local-only provider stream channel to `ModelChatCommand`. For CodeAgent model-loop calls, AgentService will attach a sender before invoking `chat_completion_for_purpose`. The provider adapter will request chat-completions streaming for CodeAgent-compatible calls, parse OpenAI-compatible SSE `choices[].delta.content` chunks, send each non-empty chunk through the channel, and still return a normal `ModelChatResp` assembled from the stream.

AgentService will consume the channel while the provider future is active and write each chunk as a `RunEventKind::Thought` payload with `item.type = "model_delta"`. The existing run-event SSE/WebSocket transports can then deliver deltas without new endpoints or schema changes. The final `model_inference` event will include streaming metadata such as `streaming`, `deltaChunkCount`, and `deltaTextLength`, but it will not duplicate the full answer.

## Alternatives Considered

1. Only store chunks on the final `ModelChatResp`.
   This is simpler, but it is not live streaming. It would make traces richer while leaving browser clients blind until the model call finishes.

2. Replace the model runtime API with an async stream return type.
   This is cleaner long-term, but too large for this slice because chat flow, RAG, Guardian, compaction, fallback, leases, and cost accounting already depend on the unary API.

3. Add a local channel while keeping the unary response.
   This is selected. It provides live event delivery for CodeAgent while preserving existing service contracts.

## Event Contract

Each token delta event uses the existing run-event table:

```json
{
  "runtimeMode": "model_loop",
  "item": {
    "type": "model_delta",
    "source": "provider_stream",
    "routeId": "runtime.llm.code_agent",
    "provider": "openai_compatible",
    "model": "gpt-...",
    "deltaIndex": 0,
    "content": "partial text"
  }
}
```

The final inference event remains a `model_inference` item and adds:

```json
{
  "streaming": true,
  "deltaChunkCount": 3,
  "deltaTextLength": 42
}
```

## Scope

This slice covers CodeAgent chat-completions SSE deltas only. Responses compaction deltas, frontend rendering affordances, partial tool-call JSON parsing while streaming, and a fully stream-native model runtime API remain follow-up work.

## Testing

Backend unit tests must prove:

- CodeAgent chat-completions requests enable provider streaming.
- OpenAI-compatible SSE chat-completions deltas assemble the final answer and preserve chunk order.
- `model_inference` payload exposes streaming metadata without duplicating answer text.
- AgentService has a channel-backed delta event sink in the model loop.

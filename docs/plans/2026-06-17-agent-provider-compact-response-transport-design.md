# Agent Provider Compact Response Transport Design

## Goal

Move Novex Agent compaction beyond prompt-adapter chat completions by adding a Codex-shaped provider transport boundary for compaction requests and compaction output parsing.

## Context

Codex has two remote compaction shapes:

- `/responses/compact` unary compaction requests that return compacted history output.
- remote compaction v2 requests that send a `compaction_trigger` item through the Responses stream and require exactly one compaction output item before `response.completed`.

Novex currently records remote compaction request metadata and passes a provider metadata envelope, but the actual provider call is still a chat-completions prompt. The next hard gap in the migration matrix is a dedicated provider compact transport and Responses-style output parser.

## Options

### Option A: Full Codex Direct Port

Port Codex `ResponseItem`, `Prompt`, `ResponseStream`, retry, and compact-history processing into Novex now.

This gives the closest parity, but it is too large for a safe slice because Novex currently has its own lightweight `ModelChatCommand` contract and Agent runtime state. It would touch protocol, runtime, trace, model service, and frontend event expectations at once.

### Option B: Provider Transport Adapter

Keep the Novex `ModelChatCommand` boundary, but when the command is a compaction request and the route can support Responses-compatible transport:

- build a Responses-compatible compaction payload with message input items and a `compaction_trigger`;
- post to the provider Responses endpoint instead of chat completions;
- parse JSON or SSE-style Responses output and require exactly one `compaction` item;
- return that compacted content through the existing `ModelChatResp` so AgentService fallback and trace paths remain stable.

This is the recommended slice. It moves provider I/O into a Codex-shaped contract while keeping Novex's enterprise control plane stable.

### Option C: Keep Metadata-Only

Leave provider calls as chat completions and only enrich trace/eval metadata.

This is already implemented and no longer advances the hard gap. It keeps compaction indistinguishable at the actual provider transport boundary.

## Selected Design

Implement Option B.

`model_service.rs` adds an internal provider request plan:

- `ChatCompletions` keeps the current endpoint, payload, and response parser.
- `ResponsesCompactionV2` is selected only when `ModelChatCommand.request_metadata.request_kind = compaction` and the route is Responses-compatible.

Responses-compatible routes are intentionally conservative:

- `openai-compatible`
- `local-runtime`

Other providers, including DeepSeek and Azure OpenAI, continue through the prompt-adapter chat-completions path for now.

The Responses compaction payload uses:

- `model`
- `input` message items converted from Novex chat messages
- final `{ "type": "compaction_trigger" }`
- `metadata` from the existing Codex-style compaction envelope
- `stream = true`
- `max_output_tokens` from `maxTokens`
- `temperature`

The endpoint is derived from `route.base_url()` plus `/responses`, so a route configured with `https://llm.example.com/v1` posts to `https://llm.example.com/v1/responses`.

## Parser Contract

The parser accepts:

- non-stream JSON bodies with an `output` array containing exactly one item whose `type` is `compaction` or `compaction_summary`;
- SSE-style text records containing `response.output_item.done` events and one `response.completed` event.

It rejects:

- zero compaction items;
- more than one compaction item;
- SSE streams that close before `response.completed`;
- compaction items without non-empty `encrypted_content`.

For Novex's current runtime, the compaction item's `encrypted_content` becomes `ModelChatResp.answer`. AgentService already normalizes JSON/plain-text compaction answers, so this preserves deterministic fallback behavior while making the provider boundary explicit.

## Acceptance

- Model-service source contracts prove compaction requests use a provider request plan and do not reuse ordinary chat payload construction.
- Payload tests prove Responses compaction input includes `compaction_trigger`, `metadata.request_kind = compaction`, and `max_output_tokens`.
- Endpoint tests prove compatible compaction routes use `/responses` while unsupported providers stay on chat completions.
- Parser tests prove JSON and SSE compaction outputs are accepted, unrelated output items are ignored, duplicate/missing compaction items are rejected, and incomplete SSE streams are rejected.
- Agent remote compaction tests remain green.
- Migration matrix records provider compact transport contract progress while leaving full Codex `ResponseItem` history installation, WebSocket streaming, and provider-native cancel endpoints as follow-ups.

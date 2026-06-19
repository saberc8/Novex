# Agent Provider Responses Output Text Delta Plan

## Goal

Add OpenAI Responses-style text streaming support to the existing CodeAgent provider delta pipeline by parsing `response.output_text.delta` SSE events into the same `ModelProviderStreamChunk` contract used by chat-completions token deltas.

## Current State

Novex already streams CodeAgent chat-completions chunks from provider SSE into a local provider stream channel. AgentService drains that channel into durable `model_delta` run events, and the frontend renders those events as live model output. Responses compaction also has JSON/SSE final-result parsing, response id/status capture, provider-call lease evidence, and native cancel controls.

The missing piece is that Responses API semantic text events are not yet recognized by the shared provider delta parser. A Responses stream that emits `response.output_text.delta` can complete successfully, but its incremental text is not converted into ordered provider chunks.

OpenAI's current streaming guide describes Responses streaming as typed SSE events and lists `response.created`, `response.output_text.delta`, and `response.completed` as common text-stream events:
https://platform.openai.com/docs/guides/streaming-responses

## Selected Approach

Extend the provider SSE parser so it accepts both formats:

- Chat Completions: `choices[].delta.content`, `choices[].message.content`, and compatible `choices[].text`.
- Responses: `type = "response.output_text.delta"` with a string `delta` field.

For Responses streams, the parser should:

- Preserve raw delta text, including leading whitespace.
- Assign monotonically increasing local chunk indexes.
- Store `provider_event = "response.output_text.delta"` on each chunk.
- Treat `response.completed` as terminal.
- Read response id/status and usage from nested `response` payloads when present.
- Build the final answer from ordered deltas, while still accepting a final `response.output` body as a fallback for unary or non-delta responses.

This keeps the unary `ModelChatResp` contract intact while allowing the already-built CodeAgent provider stream channel, durable run events, trace/eval tags, and frontend panels to work for Responses text deltas.

## Out Of Scope

- Switching configured CodeAgent routes from chat-completions to Responses by default.
- Adding a first-class route transport configuration column.
- Streaming partial tool-call JSON / function-call arguments.
- Replacing the model runtime API with an async stream return type.

Those remain follow-up slices so existing configured chat-completions providers continue to work.

## Tests

Backend unit tests should prove:

- A Responses SSE stream with two `response.output_text.delta` events assembles `answer = "Hello world"`.
- The same stream preserves chunk order, chunk indexes, raw whitespace, and provider event names.
- `response.completed.response.usage` maps to normalized prompt/completion/total tokens.
- Nested `response.id` and `response.status` are captured as provider response metadata.
- An incomplete Responses stream is rejected.

## Verification

Run the focused backend test first:

```bash
cargo test -p backend provider_token_delta_responses --offline
```

Then run the standard slice verification:

```bash
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
```

After merge to `main`, rerun the same verification on `main`, run `cargo clean` in main and feature worktrees, and fast-forward the feature branch to `main`.

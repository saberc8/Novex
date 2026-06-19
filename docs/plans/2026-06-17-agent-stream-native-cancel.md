# Agent Stream Native Cancel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When the Agent model loop early-stops after detecting a complete streamed tool call, dispatch provider-native remote cancellation using the captured stream response id and the existing provider-call lease controls.

**Architecture:** Extend provider stream events with the durable provider-call lease id that is already created before model sampling. Preserve the response id/status captured from streaming SSE records, carry lease id through Agent stream state and trace payloads, and invoke `ModelRuntimeService`'s lease cancellation path after streamed tool-call early-stop. The cancellation attempt must be non-blocking for Agent progress: success records lease-native cancel evidence; failure is logged and emitted as a runtime/inference event without discarding the parsed tool call.

**Tech Stack:** Rust, Tokio mpsc provider stream channel, backend model runtime provider-call leases, Agent model-loop streaming parser, existing Responses `/responses/{id}/cancel` transport.

## Global Constraints

- Do not add a parallel provider-cancel implementation in Agent service; reuse model runtime provider-call lease controls.
- Do not require streamed tool-call early-stop to have a provider response id; missing metadata must preserve the existing local early-stop behavior.
- Do not fail the Agent run only because provider-native remote cancellation fails after a streamed tool call has already been parsed.
- Keep stream payload additions backward compatible by only adding optional fields when metadata exists.
- Preserve existing HTTP provider-call lease cancel behavior.

---

### Task 1: Carry Provider Call Lease Id In Stream Events

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Changes: `ModelChatCommand` gains local-only `provider_call_lease_id: Option<i64>`.
- Changes: `ModelProviderStreamEvent` gains `provider_call_lease_id: Option<i64>`.
- Produces: `model_delta` and `model_stream_tool_call` payloads can include `providerCallLeaseId`.

- [ ] **Step 1: Write the failing stream lease id tests**

Add tests named `provider_stream_event_carries_provider_call_lease_id` and `provider_stream_lease_id_is_added_to_model_delta_payload`. The first should build a `ModelChatCommand` with `provider_call_lease_id: Some(4242)`, emit an SSE text delta, and assert the received `ModelProviderStreamEvent` has the lease id. The second should assert `model_delta_event_payload_from_stream_event` writes `providerCallLeaseId`.

- [ ] **Step 2: Run focused tests red**

Run: `cargo test -p backend provider_stream_lease_id --offline`

Expected: FAIL because stream events do not carry provider-call lease ids.

- [ ] **Step 3: Implement lease id propagation**

Add the local-only field to `ModelChatCommand`, set it to `Some(lease_id)` inside `execute_normalized_chat_completion_with_provider_call_lease` before invoking the provider, stamp emitted `ModelProviderStreamEvent` records with the field, retain it in `ModelLoopProviderStreamState`, and write `providerCallLeaseId` into Agent streaming payload helpers.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend provider_stream_lease_id --offline`

Expected: PASS.

---

### Task 2: Dispatch Native Cancel After Streamed Tool-Call Early Stop

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `crates/novex-eval/src/lib.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Adds: `ModelRuntimeService::cancel_provider_call_lease_with_response_metadata(user_id, lease_id, provider_response_id)`.
- Adds: Agent runtime event payload type `provider_native_cancel` for trace/eval evidence.

- [ ] **Step 1: Write failing cancellation dispatch tests**

Add source-contract tests that assert streamed tool-call completion retains `provider_call_lease_id`, `execute_model_loop_existing_run` invokes a helper after `StreamedToolCallDetected`, and the helper calls `cancel_provider_call_lease_with_response_metadata`. Add model-service tests proving the new cancel plan prefers an early stream response id over empty persisted response payloads.

- [ ] **Step 2: Run focused tests red**

Run:

```bash
cargo test -p backend provider_stream_native_cancel --offline
cargo test -p backend provider_call_lease_cancel --offline
```

Expected: FAIL because the early-stop path does not dispatch provider-native cancellation and the lease cancel helper cannot use in-memory stream metadata.

- [ ] **Step 3: Implement cancellation dispatch**

Refactor the native cancel planner to accept an optional provider response id override. Add `cancel_provider_call_lease_with_response_metadata` that loads the lease row, resolves the route, builds a cancel plan from the override before falling back to persisted payloads, executes native cancel when supported, and completes the running lease with native-cancel evidence. In Agent service, call a non-fatal `try_cancel_streamed_provider_call` helper immediately after `StreamedToolCallDetected` completion is received.

- [ ] **Step 4: Add trace/eval evidence**

Emit a `provider_native_cancel` runtime/inference payload on successful dispatch and `provider_native_cancel_error` payload on failed dispatch. Include the new inference item types in trace conversion and add eval tags for native-cancel attempt/support status.

- [ ] **Step 5: Update matrix and verify**

Update the Runtime loop row so provider-native remote cancellation dispatch from early stream response metadata is implemented; leave fully stream-native model runtime API as the next gap.

Run:

```bash
cargo fmt -- --check
cargo test -p backend provider_stream_lease_id --offline
cargo test -p backend provider_stream_native_cancel --offline
cargo test -p backend provider_call_lease_cancel --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test -p novex-eval provider_native_cancel --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands pass.

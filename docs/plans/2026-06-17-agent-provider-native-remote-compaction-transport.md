# Agent Provider-Native Remote Compaction Transport Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Carry Codex-style compaction request metadata through Novex model runtime calls so Agent remote compaction is visible at the provider transport boundary.

**Architecture:** Add an opt-in `ModelChatCommand` metadata envelope, serialize it into provider payloads for compatible routes, and have `AgentService` pass and persist the same envelope for context compaction calls. Keep unsupported providers on the existing prompt-adapter path.

**Tech Stack:** Rust, serde, serde_json, Cargo offline tests.

---

### Task 1: Model Runtime Transport Metadata

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests for:

- ordinary chat payload omits `metadata`;
- OpenAI-compatible compaction payload includes `metadata.request_kind = compaction`;
- metadata contains `compaction_implementation = responses_compaction_v2`;
- DeepSeek payload omits provider metadata even when command carries compaction metadata.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust model_chat_payload --offline
```

Expected: FAIL because `ModelChatCommand` has no request metadata contract.

**Step 3: Implement model metadata types**

Add:

- `ModelChatRequestKind`
- `ModelChatCompactionMetadata`
- `ModelChatRequestMetadata`
- `ModelChatRequestMetadata::remote_compaction(...)`

Add `request_metadata` to `ModelChatCommand`.

**Step 4: Serialize compatible provider metadata**

Update `model_chat_request_payload(...)` to add a `metadata` object only when:

- `command.request_metadata` is `Some(...)`;
- route provider is `OpenAiCompatible`, `AzureOpenAi`, or `LocalRuntime`.

**Step 5: Verify GREEN**

Run:

```bash
cargo test -p backend-rust model_chat_payload --offline
```

Expected: PASS.

### Task 2: Agent Compaction Adapter Wiring

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests proving:

- `model_loop_context_compaction_outcome(...)` passes `request_metadata` into `ModelChatCommand`;
- compaction event payload records `modelRequestMetadata`;
- compaction event payload records `compactionTransport`.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust remote_compaction --offline
```

Expected: FAIL until AgentService builds and records the model request metadata.

**Step 3: Implement adapter**

Import the new model metadata types, map `AgentRemoteCompactionRequest` into `ModelChatRequestMetadata::remote_compaction(...)`, pass it to the compaction model call, and add it to compaction event payloads.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust remote_compaction --offline
cargo test -p backend-rust model_loop_compaction --offline
```

Expected: PASS.

### Task 3: Documentation And Verification

Status: Completed.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-provider-native-remote-compaction-transport.md`

**Step 1: Update migration matrix**

Mark runtime-loop remote compaction transport metadata as implemented. Leave dedicated provider endpoint/streaming parser and cross-process provider abort as next work.

**Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

**Step 3: Commit, merge, clean**

Commit feature worktree, merge into `main`, rerun verification on `main`, and run `cargo clean` in both worktrees.

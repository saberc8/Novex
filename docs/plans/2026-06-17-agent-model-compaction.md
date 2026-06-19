# Agent Model Compaction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a model-assisted context compaction adapter to `runtimeMode=model_loop` with deterministic fallback and eval-visible compaction evidence.

**Architecture:** `novex-agent-runtime` exposes deterministic compaction candidates and custom summary installation. `AgentService` uses the configured `code_agent` model route to rewrite the candidate summary, falls back on errors, persists one `ContextCompaction` event with strategy/status metadata, and keeps the compacted loop moving. `novex-eval` reads compaction strategy tags from trace bundles.

**Tech Stack:** Rust, Cargo offline tests, existing `ModelRuntimeService`, `novex-agent-runtime`, `novex-trace`, `novex-eval`.

---

### Task 1: Runtime Custom Compaction Summary Contract

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Step 1: Write the failing test**

Add in `crates/novex-agent-runtime/src/lib.rs` tests:

```rust
#[test]
fn runtime_compaction_can_install_model_generated_summary() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 4,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::user_message("summarize refund policy"));
    state.push_item(AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        json!({"hits":[{"text":"refund within 7 days"}]}),
    ));

    let candidate = state.compaction_candidate_summary().unwrap();
    assert!(candidate.contains("refund within 7 days"));

    let compaction = state
        .compact_context_with_summary("Model summary: refunds are allowed within 7 days.")
        .unwrap();

    assert_eq!(compaction.window_id, 1);
    assert_eq!(
        compaction.summary,
        "Model summary: refunds are allowed within 7 days."
    );
    assert!(!state.should_compact_context());
    assert!(matches!(
        state.items.last(),
        Some(AgentTurnItem::ContextCompaction { summary })
            if summary == "Model summary: refunds are allowed within 7 days."
    ));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-agent-runtime runtime_compaction_can_install_model_generated_summary --offline
```

Expected: FAIL because `compaction_candidate_summary` and `compact_context_with_summary` do not exist.

**Step 3: Implement minimal runtime support**

Add:

```rust
pub fn compaction_candidate_summary(&self) -> Option<String>
pub fn compact_context_with_summary(&mut self, summary: impl Into<String>) -> Option<AgentContextCompaction>
```

Refactor existing `compact_context()` to:

```rust
pub fn compact_context(&mut self) -> Option<AgentContextCompaction> {
    let summary = self.compaction_candidate_summary()?;
    self.compact_context_with_summary(summary)
}
```

The custom summary path must still:

- Check `should_compact_context()`.
- Advance `compaction_window_id`.
- Append `AgentTurnItem::ContextCompaction`.
- Preserve `retained_item_count` and `compacted_item_count`.

**Step 4: Run focused and package tests**

Run:

```bash
cargo test -p novex-agent-runtime runtime_compaction --offline
cargo test -p novex-agent-runtime --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-agent-runtime/src/lib.rs
git commit -m "feat: allow model generated compaction summaries"
```

### Task 2: Backend Model Compaction Adapter

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add in `agent_service.rs` tests:

```rust
#[test]
fn model_loop_compaction_prompt_uses_deterministic_candidate_and_tool_context() {
    let tool_codes = vec!["rag.search".to_owned(), "github.repo.read".to_owned()];

    let messages = build_model_loop_context_compaction_messages(
        "Find refund policy",
        "Observation for call-1: refund within 7 days",
        &tool_codes,
    );

    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("Novex Agent Context Compactor"));
    assert!(messages[1].content.contains("Find refund policy"));
    assert!(messages[1].content.contains("refund within 7 days"));
    assert!(messages[1].content.contains("rag.search, github.repo.read"));
}

#[test]
fn model_loop_model_compaction_response_accepts_json_or_plain_text() {
    assert_eq!(
        model_loop_context_compaction_summary_from_response(r#"{"summary":"short policy"}"#),
        "short policy"
    );
    assert_eq!(
        model_loop_context_compaction_summary_from_response("plain short policy"),
        "plain short policy"
    );
}

#[test]
fn agent_service_model_loop_uses_model_assisted_context_compaction() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("runtime_state.compaction_candidate_summary()"));
    assert!(source.contains("model_loop_context_compaction_outcome"));
    assert!(source.contains("chat_completion_for_purpose(ModelRoutePurpose::CodeAgent"));
    assert!(source.contains("runtime_state.compact_context_with_summary"));
    assert!(source.contains("\"compactionStrategy\""));
    assert!(source.contains("\"compactionStatus\""));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend model_loop_compaction --offline
```

Expected: FAIL because model compaction helper functions and source wiring do not exist.

**Step 3: Add helper types and prompt builders**

Add:

```rust
#[derive(Debug, Clone)]
struct ModelLoopContextCompactionOutcome {
    summary: String,
    strategy: String,
    status: String,
    cancelled: bool,
    model_payload: Option<Value>,
    error_payload: Option<Value>,
    error_message: Option<String>,
}
```

Add:

```rust
fn build_model_loop_context_compaction_messages(
    original_input: &str,
    deterministic_summary: &str,
    tool_codes: &[String],
) -> Vec<ModelChatMessage>

fn model_loop_context_compaction_summary_from_response(answer: &str) -> String
```

The response helper should accept either JSON `{ "summary": "..." }` or plain text, trim blank values, and fall back to the original text when JSON parsing fails.

**Step 4: Add adapter method**

Add an `AgentService` method:

```rust
async fn model_loop_context_compaction_outcome(
    &self,
    cancel_token: AgentRunCancellationToken,
    original_input: &str,
    deterministic_summary: &str,
    tool_codes: &[String],
) -> Result<ModelLoopContextCompactionOutcome, AppError>
```

It should call:

```rust
self.model_runtime.chat_completion_for_purpose(
    ModelRoutePurpose::CodeAgent,
    ModelChatCommand {
        messages: build_model_loop_context_compaction_messages(...),
        temperature: Some(0.1),
        max_tokens: Some(512),
        ..ModelChatCommand::default()
    },
)
```

through `await_model_loop_future_or_cancelled`.

Outcomes:

- Completed: `strategy = "model"`, `status = "succeeded"`, `summary` from model response, `model_payload = Some(model_inference_event_payload(&response)["item"].clone())`.
- Error: `strategy = "deterministic_fallback"`, `status = "fallback_used"`, `summary = deterministic_summary`, `error_payload = Some(model_inference_error_event_payload(&err, latency_ms)["item"].clone())`.
- Cancelled: `strategy = "model"`, `status = "cancelled"`, `cancelled = true`, `summary = deterministic_summary`.

**Step 5: Wire into model loop**

Replace direct `runtime_state.compact_context()` usage with:

```rust
if runtime_state.should_compact_context() {
    if let Some(deterministic_summary) = runtime_state.compaction_candidate_summary() {
        let compaction_outcome = self
            .model_loop_context_compaction_outcome(
                cancel_token.clone(),
                &command.input,
                &deterministic_summary,
                &tool_codes,
            )
            .await?;
        if compaction_outcome.cancelled {
            if self
                .check_model_loop_cancelled(user_id, run_id, "context_compaction")
                .await?
                == ModelLoopCancelCheck::Continue
            {
                self.finish_model_loop_cancelled(
                    user_id,
                    run_id,
                    &run_status_code(RunStatus::Cancelling),
                    "context_compaction",
                )
                .await?;
            }
            return self.get_run(run_id).await;
        }
        if let Some(compaction) =
            runtime_state.compact_context_with_summary(compaction_outcome.summary.clone())
        {
            // existing ContextCompaction payload plus strategy/status/model/error metadata
        }
    }
}
```

The `ContextCompaction` payload must include:

- `compactionStrategy`
- `compactionStatus`
- `modelInference` when present
- `modelError` when present
- `errorMessage` when present

**Step 6: Run focused tests**

Run:

```bash
cargo test -p backend model_loop_compaction --offline
cargo test -p backend model_loop --offline
```

Expected: PASS.

**Step 7: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: add model assisted agent context compaction"
```

### Task 3: Eval Compaction Evidence Tags

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing eval test**

Add in `novex-eval` tests:

```rust
#[test]
fn trace_eval_candidate_tags_model_compaction_strategy() {
    let bundle = TraceBundle::new("trace-compact")
        .with_event(TraceEvent::user_message(1, "answer from a long notebook"))
        .with_event(TraceEvent::context_compaction(
            2,
            json!({
                "item": {"type":"context_compaction","summary":"model summary"},
                "compactionStrategy": "model",
                "compactionStatus": "succeeded"
            }),
        ))
        .with_event(TraceEvent::final_answer(3, "done"));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["compactionCount"], 1);
    assert_eq!(candidate.tags["modelCompactionCount"], 1);
    assert_eq!(candidate.tags["compactionFallbackCount"], 0);
    assert_eq!(candidate.tags["compactionStatus"], "succeeded");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-eval trace_eval_candidate_tags_model_compaction_strategy --offline
```

Expected: FAIL because tags are not emitted.

**Step 3: Implement tag extraction**

Add a small helper that scans `TraceEventKind::ContextCompaction` payloads and counts:

- `compactionStrategy == "model"` as `modelCompactionCount`
- `compactionStrategy == "deterministic_fallback"` as `compactionFallbackCount`
- last non-empty `compactionStatus` as `compactionStatus`

Add those tags in `EvalCaseCandidate::from_trace_bundle_with_policy` only when `compaction_count > 0`.

**Step 4: Run eval tests**

Run:

```bash
cargo test -p novex-eval compaction --offline
cargo test -p novex-eval --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs
git commit -m "feat: tag trace eval compaction strategy"
```

### Task 4: Matrix and Final Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-model-compaction.md`

**Step 1: Update matrix**

Change Runtime loop row from `slice-6 implemented` to `slice-17 implemented`, noting:

- model-assisted context compaction adapter
- deterministic fallback on compaction model failure
- compaction strategy/status metadata for trace/eval
- remote compact endpoint parity and supervised background workers remain next

Update Runtime loop POC verification command to include:

```bash
cargo test -p novex-agent-runtime --offline && cargo test -p backend model_loop_compaction --offline && cargo test -p backend model_loop --offline
```

Update Rollout/trace/eval acceptance command to include:

```bash
cargo test -p novex-eval compaction --offline
```

Add the plan link:

```markdown
- Agent model compaction: `docs/plans/2026-06-17-agent-model-compaction.md`
```

**Step 2: Verify full slice**

Run:

```bash
cargo fmt -- --check
cargo test -p novex-agent-runtime runtime_compaction --offline
cargo test -p backend model_loop_compaction --offline
cargo test -p backend model_loop --offline
cargo test -p backend runtime_spans --offline
cargo test -p novex-eval compaction --offline
cargo test --workspace --offline
```

Expected: all pass.

**Step 3: Commit docs**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-model-compaction.md
git commit -m "docs: record model assisted compaction progress"
```

**Step 4: Merge to main**

After feature verification:

```bash
cd /path/to/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation model compaction"
cargo fmt -- --check
cargo test --workspace --offline
cd /path/to/Novex/.worktrees/enterprise-agent-foundation
git merge --ff-only main
```

Expected: main and feature worktree both point at the merge commit and are clean.

# Agent Streaming Tool Call Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable Agent runtime parser that can accumulate streamed model deltas and emit a parsed tool-call turn as soon as a complete JSON `tool_call` or `tool_calls` object is available.

**Architecture:** Keep the existing `parse_model_turn_output` full-answer parser as the canonical schema parser. Add a small streaming wrapper in `crates/novex-agent-runtime` that buffers provider text deltas, recognizes JSON object boundaries via `serde_json`, returns `Pending` while the JSON is incomplete, and returns `Ready(ParsedModelTurnOutput)` only when the completed JSON parses as `tool_call` or `tool_calls`. Plain final-answer text stays pending in this slice so consumers do not accidentally treat a partial natural-language answer as terminal.

**Tech Stack:** Rust, `serde_json`, `novex-agent-protocol`, existing `ParsedModelTurnOutput` and `ModelTurnParseError`.

## Global Constraints

- Reuse `parse_model_turn_output` for final tool-call schema validation.
- Do not change backend model-loop execution order in this slice.
- Do not classify streamed natural-language text as final answer.
- Preserve the compact JSON contract used by the existing model-loop prompt.
- Keep parser memory bounded.

---

### Task 1: Streaming Single Tool Call Parser

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Interfaces:**
- Produces: `StreamingModelTurnParser::new()`.
- Produces: `StreamingModelTurnParser::push_delta(&mut self, delta: &str) -> Result<StreamingModelTurnParseStatus, ModelTurnParseError>`.
- Produces: `StreamingModelTurnParseStatus::{Pending, Ready(ParsedModelTurnOutput)}`.

- [ ] **Step 1: Write the failing single-call test**

Add this test near the existing parser tests:

```rust
#[test]
fn streaming_parser_waits_for_complete_tool_call_json() {
    let mut parser = StreamingModelTurnParser::new();

    assert_eq!(
        parser.push_delta(r#"{"type":"tool_"#).unwrap(),
        StreamingModelTurnParseStatus::Pending
    );
    let status = parser
        .push_delta(
            r#"call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#,
        )
        .unwrap();

    match status {
        StreamingModelTurnParseStatus::Ready(parsed) => {
            assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
            assert_eq!(parsed.items.len(), 1);
            assert_eq!(
                parsed.item,
                AgentTurnItem::tool_call(
                    "call-1",
                    "rag.search",
                    json!({"query":"policy"})
                )
            );
        }
        StreamingModelTurnParseStatus::Pending => panic!("expected complete tool call"),
    }
}
```

- [ ] **Step 2: Run red verification**

Run:

```bash
cargo test -p novex-agent-runtime streaming_parser_waits_for_complete_tool_call_json --offline
```

Expected: FAIL because `StreamingModelTurnParser` and `StreamingModelTurnParseStatus` do not exist.

- [ ] **Step 3: Implement minimal streaming parser**

Add public types:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum StreamingModelTurnParseStatus {
    Pending,
    Ready(ParsedModelTurnOutput),
}

#[derive(Debug, Clone)]
pub struct StreamingModelTurnParser {
    buffer: String,
    ready: bool,
    max_chars: usize,
}
```

Implement:

- `new()` with a bounded default such as `MAX_STREAMING_MODEL_TURN_BUFFER_CHARS`.
- `push_delta` appends raw text, trims only for parsing, returns `Pending` for incomplete JSON EOF, and returns `Ready(parse_model_turn_output(trimmed)?)` for completed `tool_call` / `tool_calls` JSON.
- Once ready, additional deltas return a parse error.

- [ ] **Step 4: Run green verification**

Run:

```bash
cargo test -p novex-agent-runtime streaming_parser_waits_for_complete_tool_call_json --offline
```

Expected: PASS.

### Task 2: Streaming Batch And Text Boundaries

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Interfaces:**
- Extends `StreamingModelTurnParser` behavior.

- [ ] **Step 1: Add batch and text-boundary tests**

Add tests proving:

- `tool_calls` JSON split across chunks returns `Ready` with two `AgentTurnItem::ToolCall` entries.
- natural-language deltas return `Pending` and do not become `FinalAnswer`.
- oversized buffers return a `ModelTurnParseError`.

- [ ] **Step 2: Run red/green focused verification**

Run:

```bash
cargo test -p novex-agent-runtime streaming_parser --offline
```

Expected: PASS after minimal implementation.

### Task 3: Matrix Update And Integration

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Updates Runtime loop row and Runtime loop POC acceptance evidence.

- [ ] **Step 1: Update matrix wording**

Move "partial tool-call JSON parsing while streaming" into implemented evidence as a runtime parser capability, and leave "backend stream-native execution of parsed tool calls" as follow-up work.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit and integrate**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main` with `--no-ff`, rerun verification on `main`, run `cargo clean` in both worktrees, and fast-forward the feature branch to `main`.

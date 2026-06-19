# Novex Agent Runtime Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-agent-runtime` from a single `src/lib.rs` into focused runtime state and model-turn parser modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade and move behavior unchanged into `state` for budget/context-compaction runtime state and `parser` for model turn parsing and streaming parser logic. Move inline tests into integration tests by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `novex-agent-protocol`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new agent runtime behavior.
- Preserve root-level exports such as `novex_agent_runtime::AgentRuntimeState`, `novex_agent_runtime::AgentRuntimeBudget`, `novex_agent_runtime::parse_model_turn_output`, and `novex_agent_runtime::StreamingModelTurnParser`.
- Keep cross-crate dependency direction as `novex-agent-runtime -> novex-agent-protocol`.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-agent-runtime`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-agent-runtime/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-agent-runtime/src/state.rs`
  - Owns runtime budget, context compaction state, remote compaction request DTOs, `AgentRuntimeState`, and compaction helpers.
- Create: `crates/novex-agent-runtime/src/parser.rs`
  - Owns parsed model-turn output, parse errors, streaming parser, parsing constants, and JSON tool-call parsing helpers.
- Modify: `crates/novex-agent-runtime/src/lib.rs`
  - Keep only module declarations, root re-exports, and `CRATE_ID`.

---

### Task 1: Add Runtime Structure and Public-Facade Characterization Tests

**Files:**
- Create: `crates/novex-agent-runtime/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_agent_runtime`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-agent-runtime/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};
use novex_agent_runtime::{
    parse_model_turn_output, AgentCompactionReason, AgentCompactionTrigger,
    AgentRemoteCompactionImplementation, AgentRuntimeBudget, AgentRuntimeState,
    StreamingModelTurnParseStatus, StreamingModelTurnParser, MAX_STREAMING_MODEL_TURN_BUFFER_CHARS,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_agent_runtime_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["parser", "state"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct AgentRuntimeBudget",
        "pub struct AgentRuntimeState",
        "pub struct ParsedModelTurnOutput",
        "pub struct StreamingModelTurnParser",
        "pub fn parse_model_turn_output",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn agent_runtime_domain_modules_exist() {
    for module in ["src/parser.rs", "src/state.rs"] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_runtime_state_and_parser_contracts() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 2,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::user_message("find policy"));
    state.push_item(AgentTurnItem::tool_call(
        "call-1",
        "rag.search",
        serde_json::json!({"query":"policy"}),
    ));
    state.push_item(AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        serde_json::json!({"hits":[]}),
    ));

    assert_eq!(state.next_outcome(), TurnOutcome::NeedsFollowUp);
    assert!(state.should_compact_context());
    let request = state
        .remote_compaction_request(vec!["rag.search".to_owned()])
        .unwrap();
    assert_eq!(request.implementation, AgentRemoteCompactionImplementation::ResponsesCompactionV2);
    assert_eq!(request.trigger, AgentCompactionTrigger::Auto);
    assert_eq!(request.reason, AgentCompactionReason::ObservationThreshold);

    let parsed = parse_model_turn_output(
        r#"{"type":"tool_call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#,
    )
    .unwrap();
    assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);

    let mut streaming = StreamingModelTurnParser::with_max_chars(MAX_STREAMING_MODEL_TURN_BUFFER_CHARS);
    assert_eq!(
        streaming.push_delta("plain text").unwrap(),
        StreamingModelTurnParseStatus::Pending
    );
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-agent-runtime --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Runtime Source and Tests

**Files:**
- Create: `crates/novex-agent-runtime/src/state.rs`
- Create: `crates/novex-agent-runtime/src/parser.rs`
- Create: `crates/novex-agent-runtime/tests/state.rs`
- Create: `crates/novex-agent-runtime/tests/parser.rs`
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Interfaces:**
- Consumes: existing `crates/novex-agent-runtime/src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move runtime state**

Move these items into `src/state.rs`:

```rust
AgentRuntimeBudget
AgentContextCompaction
AgentRemoteCompactionImplementation
AgentCompactionTrigger
AgentCompactionReason
AgentCompactionPhase
AgentRemoteCompactionRequest
AgentRuntimeState
retained_remote_compaction_history
build_compaction_summary
compact_text
```

`state.rs` should import:

```rust
use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};
```

- [ ] **Step 2: Move parser**

Move these items into `src/parser.rs`:

```rust
ParsedModelTurnOutput
ModelTurnParseError
MAX_STREAMING_MODEL_TURN_BUFFER_CHARS
StreamingModelTurnParseStatus
StreamingModelTurnParser
impl StreamingModelTurnParser
impl Default for StreamingModelTurnParser
parse_model_turn_output
parse_tool_call_value
```

`parser.rs` should import:

```rust
use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;
```

- [ ] **Step 3: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod parser;
mod state;

pub use parser::{
    parse_model_turn_output, ModelTurnParseError, ParsedModelTurnOutput,
    StreamingModelTurnParseStatus, StreamingModelTurnParser, MAX_STREAMING_MODEL_TURN_BUFFER_CHARS,
};
pub use state::{
    AgentCompactionPhase, AgentCompactionReason, AgentCompactionTrigger, AgentContextCompaction,
    AgentRemoteCompactionImplementation, AgentRemoteCompactionRequest, AgentRuntimeBudget,
    AgentRuntimeState,
};

pub const CRATE_ID: &str = "novex-agent-runtime";
```

- [ ] **Step 4: Move test groups**

Use `use novex_agent_runtime::*;`, `use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};`, and `use serde_json::json;` in integration tests.

Move tests according to this map:

```text
runtime_state_continues_after_observation -> tests/state.rs
runtime_budget_stops_excessive_tool_calls -> tests/state.rs
runtime_budget_allows_tool_calls_up_to_limit -> tests/state.rs
runtime_budget_exceeds_when_tool_calls_reach_limit_before_next_call -> tests/state.rs
runtime_budget_reports_remaining_tool_call_capacity -> tests/state.rs
runtime_compaction_is_needed_after_observation_threshold -> tests/state.rs
runtime_compaction_pushes_summary_and_advances_window -> tests/state.rs
runtime_compaction_can_install_model_generated_summary -> tests/state.rs
remote_compaction_request_exposes_endpoint_metadata -> tests/state.rs
remote_compaction_request_retains_user_and_previous_summary -> tests/state.rs
parser_reads_json_tool_call_from_model_answer -> tests/parser.rs
streaming_parser_waits_for_complete_tool_call_json -> tests/parser.rs
streaming_parser_reads_tool_call_batch_across_chunks -> tests/parser.rs
streaming_parser_keeps_natural_language_pending -> tests/parser.rs
streaming_parser_rejects_oversized_buffer -> tests/parser.rs
parser_reads_json_tool_call_batch_from_model_answer -> tests/parser.rs
parser_rejects_empty_tool_call_batch -> tests/parser.rs
parser_treats_plain_text_as_final_answer -> tests/parser.rs
```

- [ ] **Step 5: Verify `lib.rs` no longer owns tests**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-agent-runtime/src/lib.rs
```

Expected: no output and exit code 1.

- [ ] **Step 6: Run runtime tests**

Run:

```bash
cargo test -p novex-agent-runtime
```

Expected: PASS with `src/lib.rs` reporting 0 unit tests and the moved integration tests passing.

---

### Task 3: Update Runtime Source-Location Docs and Commit

**Files:**
- Modify `docs/ARCHITECTURE.md` if its `novex-agent-runtime` layout does not match the implementation.
- Modify `docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md` if the smaller-crate guidance needs a concrete path note.

**Interfaces:**
- Consumes: new runtime module paths.
- Produces: committed, verified `novex-agent-runtime` module architecture slice.

- [ ] **Step 1: Run formatting and focused verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-agent-runtime
cargo test -p backend application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-agent-runtime/src crates/novex-agent-runtime/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex agent runtime into focused modules"
```

Expected: commit succeeds.

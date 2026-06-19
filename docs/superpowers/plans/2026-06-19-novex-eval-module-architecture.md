# Novex Eval Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-eval` from a 2,076-line `src/lib.rs` into focused eval modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as the crate facade and move behavior unchanged into modules for eval case DTOs, trace extraction, scoring, regression reports, and text helpers. Keep trace summary helpers private to `trace_extract`, keep scoring helpers private to `score`, and re-export the existing public API from the crate root.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `novex-ai-core`, `novex-trace`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new eval behavior.
- Preserve root-level exports such as `novex_eval::EvalCaseExpected`, `novex_eval::EvalCaseCandidate`, `novex_eval::actual_from_trace_bundle`, `novex_eval::score_case`, and `novex_eval::build_regression_report`.
- Keep cross-crate dependency direction as `novex-eval -> novex-ai-core / novex-trace`.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-eval`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-eval/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-eval/src/case.rs`
  - Owns eval target/metric enums, case input/expected/actual DTOs, trace policy, and candidate DTO shape.
- Create: `crates/novex-eval/src/trace_extract.rs`
  - Owns `EvalCaseCandidate::from_trace_bundle*`, `actual_from_trace_bundle`, and private trace summary/extraction helpers.
- Create: `crates/novex-eval/src/score.rs`
  - Owns `EvalCaseScore`, score dispatch, individual metric scoring functions, and private exact-answer scoring.
- Create: `crates/novex-eval/src/report.rs`
  - Owns `RegressionReport` and `build_regression_report`.
- Create: `crates/novex-eval/src/text.rs`
  - Owns case-insensitive matching and score rounding helpers.
- Modify: `crates/novex-eval/src/lib.rs`
  - Keep only module declarations, root re-exports, `CRATE_ID`, and `module()`.

---

### Task 1: Add Eval Structure and Public-Facade Characterization Tests

**Files:**
- Create: `crates/novex-eval/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_eval`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-eval/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_eval::{
    actual_from_trace_bundle, build_regression_report, score_case, score_cost_case,
    score_latency_case, EvalCaseActual, EvalCaseCandidate, EvalCaseExpected, EvalCaseInput,
    EvalMetricKind, EvalTargetKind, TraceEvalPolicy,
};
use novex_trace::{TraceBundle, TraceEvent};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_eval_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["case", "report", "score", "text", "trace_extract"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum EvalTargetKind",
        "pub struct EvalCaseCandidate",
        "pub fn actual_from_trace_bundle",
        "pub struct EvalCaseScore",
        "pub fn score_case",
        "pub struct RegressionReport",
        "pub fn build_regression_report",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn eval_domain_modules_exist() {
    for module in [
        "src/case.rs",
        "src/report.rs",
        "src/score.rs",
        "src/text.rs",
        "src/trace_extract.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_case_trace_and_score_contracts() {
    let input = EvalCaseInput {
        target_kind: EvalTargetKind::Rag,
        prompt: "When does training start?".to_owned(),
    };
    assert_eq!(input.target_kind, EvalTargetKind::Rag);
    assert_eq!(TraceEvalPolicy::default().answer_snippet_max_chars, 120);

    let bundle = TraceBundle::new("trace-1")
        .with_event(TraceEvent::user_message(1, "Find policy"))
        .with_event(TraceEvent::tool_call(
            2,
            "call-1",
            "rag.search",
            serde_json::json!({"query": "policy"}),
        ))
        .with_event(TraceEvent::final_answer(
            3,
            "Training starts Monday.",
            vec!["handbook:0".to_owned()],
        ));
    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);
    assert_eq!(candidate.expected.tool_code.as_deref(), Some("rag.search"));
    assert_eq!(
        actual_from_trace_bundle(&bundle).answer.as_deref(),
        Some("Training starts Monday.")
    );

    let expected = EvalCaseExpected {
        answer_contains: vec!["Monday".to_owned()],
        citations: vec!["handbook:0".to_owned()],
        intent: None,
        tool_code: None,
    };
    let actual = EvalCaseActual {
        answer: Some("Training starts Monday.".to_owned()),
        citations: vec!["handbook:0".to_owned()],
        intent: None,
        tool_code: None,
        cost_cents: 2,
        latency_ms: 10,
    };
    let score = score_case("case-1", EvalTargetKind::Rag, &expected, &actual);
    assert!(score.passed);
    assert_eq!(score.metric, EvalMetricKind::CitationAccuracy);

    let latency = score_latency_case("latency", EvalTargetKind::Rag, &actual, 20);
    assert!(latency.passed);
    let cost = score_cost_case("cost", EvalTargetKind::Rag, &actual, 2);
    assert!(cost.passed);

    let report = build_regression_report(&[score, latency, cost]);
    assert_eq!(report.total_cases, 3);
    assert_eq!(report.passed_cases, 3);
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-eval --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Eval Source Into Focused Modules

**Files:**
- Create: `crates/novex-eval/src/case.rs`
- Create: `crates/novex-eval/src/trace_extract.rs`
- Create: `crates/novex-eval/src/score.rs`
- Create: `crates/novex-eval/src/report.rs`
- Create: `crates/novex-eval/src/text.rs`
- Modify: `crates/novex-eval/src/lib.rs`

**Interfaces:**
- Consumes: existing `crates/novex-eval/src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move case DTOs**

Move these items into `src/case.rs`:

```rust
EvalTargetKind
EvalMetricKind
EvalCaseInput
EvalCaseExpected
EvalCaseActual
TraceEvalPolicy
impl Default for TraceEvalPolicy
EvalCaseCandidate
```

`case.rs` should import:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
```

- [ ] **Step 2: Move trace extraction**

Move these items into `src/trace_extract.rs`:

```rust
impl EvalCaseCandidate
actual_from_trace_bundle
trace_event_payload_text
trace_last_event_payload_text
trace_event_count
TraceCompactionSummary
trace_compaction_summary
trace_first_cancellation_reason
TraceGuardianReviewSummary
trace_guardian_review_summary
TraceRuntimeSupervisorSummary
trace_runtime_supervisor_summary
TraceToolIoTaskSummary
trace_tool_io_task_summary
trace_tool_io_task_payload
TraceInferenceSummary
trace_inference_summary
trace_inference_payload
trace_value_i64
trace_value_f64
trace_value_text
trace_value_raw_text
trace_answer_snippet
trace_bundle_citations
collect_citations_from_value
```

`trace_extract.rs` should import:

```rust
use crate::case::{
    EvalCaseActual, EvalCaseCandidate, EvalCaseExpected, EvalMetricKind, EvalTargetKind,
    TraceEvalPolicy,
};
use novex_trace::{TraceBundle, TraceEventKind};
use serde_json::{json, Value};
use std::collections::BTreeSet;
```

- [ ] **Step 3: Move scoring**

Move these items into `src/score.rs`:

```rust
EvalCaseScore
score_case
score_rag_case
score_intent_case
score_tool_case
score_customer_service_grounded_resolution_case
score_customer_service_handoff_accuracy_case
score_latency_case
score_cost_case
score_retrieval_recall_case
score_exact_answer_case
```

`score.rs` should import:

```rust
use crate::case::{EvalCaseActual, EvalCaseExpected, EvalMetricKind, EvalTargetKind};
use crate::text::contains_case_insensitive;
use serde::{Deserialize, Serialize};
```

- [ ] **Step 4: Move regression report**

Move these items into `src/report.rs`:

```rust
RegressionReport
build_regression_report
```

`report.rs` should import:

```rust
use crate::case::EvalMetricKind;
use crate::score::EvalCaseScore;
use crate::text::round_score;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
```

- [ ] **Step 5: Move text helpers**

Move these items into `src/text.rs`:

```rust
contains_case_insensitive
round_score
```

Use this visibility:

```rust
pub(crate) fn contains_case_insensitive(value: &str, needle: &str) -> bool;
pub(crate) fn round_score(score: f64) -> f64;
```

- [ ] **Step 6: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod case;
mod report;
mod score;
mod text;
mod trace_extract;

use novex_ai_core::FoundationModule;

pub use case::{
    EvalCaseActual, EvalCaseCandidate, EvalCaseExpected, EvalCaseInput, EvalMetricKind,
    EvalTargetKind, TraceEvalPolicy,
};
pub use report::{build_regression_report, RegressionReport};
pub use score::{
    score_case, score_cost_case, score_customer_service_grounded_resolution_case,
    score_customer_service_handoff_accuracy_case, score_intent_case, score_latency_case,
    score_rag_case, score_retrieval_recall_case, score_tool_case, EvalCaseScore,
};
pub use trace_extract::actual_from_trace_bundle;

pub const CRATE_ID: &str = "novex-eval";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Eval",
        "ai-foundation",
        "Eval dataset, case, runner, metrics, report, and regression boundary.",
    )
}
```

- [ ] **Step 7: Run the structure test and full crate test**

Run:

```bash
cargo test -p novex-eval --test module_structure
cargo test -p novex-eval
```

Expected: PASS.

---

### Task 3: Move Inline Tests Into Focused Integration Tests

**Files:**
- Create: `crates/novex-eval/tests/module_contract.rs`
- Create: `crates/novex-eval/tests/case.rs`
- Create: `crates/novex-eval/tests/score.rs`
- Create: `crates/novex-eval/tests/report.rs`
- Create: `crates/novex-eval/tests/trace_extract.rs`
- Modify: `crates/novex-eval/src/lib.rs`

**Interfaces:**
- Consumes: current tests in `#[cfg(test)] mod tests`.
- Produces: focused integration-test files with unchanged assertions.

- [ ] **Step 1: Move test groups**

Use root imports such as `use novex_eval::*;` in integration tests. Add `use novex_ai_core::FoundationStatus;` in `module_contract.rs`. Add `use novex_trace::{TraceBundle, TraceEvent, TraceEventKind};` and `use serde_json::json;` in trace-focused tests.

Move tests according to this map:

```text
module_describes_eval_boundary -> tests/module_contract.rs
eval_runtime_expected_payload_defaults_fields_for_intent_and_tool_cases -> tests/case.rs
eval_runtime_scores_rag_case_with_answer_and_citation_match -> tests/score.rs
eval_runtime_scores_intent_case_by_exact_match -> tests/score.rs
eval_runtime_scores_tool_case_by_selected_tool -> tests/score.rs
eval_runtime_scores_latency_case_with_max_threshold -> tests/score.rs
eval_runtime_scores_cost_case_with_max_threshold -> tests/score.rs
eval_runtime_scores_retrieval_recall_by_expected_citations -> tests/score.rs
customer_service_eval_scores_citation_and_handoff_accuracy -> tests/score.rs
eval_runtime_builds_regression_report_with_metric_breakdown -> tests/report.rs
customer_service_eval_report_flags_missing_evidence -> tests/report.rs
trace_eval_candidate_extracts_tool_and_final_answer -> tests/trace_extract.rs
trace_eval_candidate_tags_runtime_spans -> tests/trace_extract.rs
runtime_supervisor_trace_eval_candidate_tags_runtime_cancellation -> tests/trace_extract.rs
tool_io_observability_trace_eval_candidate_tags_task_metrics -> tests/trace_extract.rs
guardian_review_trace_eval_candidate_tags_approval_review -> tests/trace_extract.rs
guardian_auto_approval_trace_eval_candidate_tags_action_review -> tests/trace_extract.rs
guardian_model_review_trace_eval_candidate_tags_reviewer_metadata -> tests/trace_extract.rs
trace_eval_candidate_tags_model_compaction_strategy -> tests/trace_extract.rs
remote_compaction_trace_eval_candidate_tags_endpoint_contract -> tests/trace_extract.rs
trace_eval_candidate_tags_inference_spans -> tests/trace_extract.rs
trace_eval_candidate_tags_model_delta_streaming -> tests/trace_extract.rs
trace_eval_candidate_tags_streaming_tool_call_detection -> tests/trace_extract.rs
provider_native_cancel_trace_eval_candidate_tags_cancel_attempt -> tests/trace_extract.rs
trace_eval_candidate_tags_provider_error_spans -> tests/trace_extract.rs
trace_eval_candidate_tags_provider_retry_spans -> tests/trace_extract.rs
trace_eval_candidate_tags_provider_fallback_attempts -> tests/trace_extract.rs
trace_eval_candidate_tags_circuit_breaker_attempts -> tests/trace_extract.rs
trace_eval_actual_extracts_tool_and_final_answer -> tests/trace_extract.rs
bundle_with_tool_and_final helper -> tests/trace_extract.rs
```

- [ ] **Step 2: Verify `lib.rs` no longer owns tests**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-eval/src/lib.rs
```

Expected: no output and exit code 1.

- [ ] **Step 3: Run eval tests**

Run:

```bash
cargo test -p novex-eval
```

Expected: PASS with `src/lib.rs` reporting 0 unit tests and the moved integration tests passing.

---

### Task 4: Update Eval Source-Location Docs

**Files:**
- Modify docs reported by `rg "crates/novex-eval/src/lib.rs|novex-eval/src/lib.rs" docs/plans docs/superpowers`.
- Modify `docs/ARCHITECTURE.md` if its `crates/novex-eval/` layout does not match the implementation.

**Interfaces:**
- Consumes: new Eval module paths.
- Produces: docs that point future Eval work at focused modules instead of `src/lib.rs`.

- [ ] **Step 1: Find stale Eval `lib.rs` instructions**

Run:

```bash
rg -n 'crates/novex-eval/src/lib.rs|novex-eval/src/lib.rs' docs/plans docs/superpowers
```

Expected: matches in older plans.

- [ ] **Step 2: Update contributor-facing references**

Replace future-work instructions according to ownership:

```text
Eval case and candidate DTOs -> crates/novex-eval/src/case.rs
Trace-derived candidates and actual extraction -> crates/novex-eval/src/trace_extract.rs
Scoring functions -> crates/novex-eval/src/score.rs
Regression report aggregation -> crates/novex-eval/src/report.rs
Text/rounding helpers -> crates/novex-eval/src/text.rs
crate facade only -> crates/novex-eval/src/lib.rs
```

Do not rewrite historical skeleton creation records.

---

### Task 5: Final Verification and Commit

**Files:**
- Commit all `novex-eval` source, test, and doc changes.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-eval` module architecture slice.

- [ ] **Step 1: Run formatting check**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 2: Run focused and nearby eval verification**

Run:

```bash
cargo test -p novex-eval
cargo test -p novex-trace
cargo test -p backend application::ai::foundation_service::tests::summary_lists_required_foundation_crates
```

Expected: PASS.

- [ ] **Step 3: Run diff check**

Run:

```bash
git diff --check
```

Expected: PASS.

- [ ] **Step 4: Commit the slice**

Run:

```bash
git add crates/novex-eval/src crates/novex-eval/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex eval into focused modules"
```

Expected: commit succeeds.

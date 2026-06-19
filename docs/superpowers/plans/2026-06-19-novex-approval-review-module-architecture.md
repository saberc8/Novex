# Novex Approval Review Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-approval-review` from a single `src/lib.rs` into focused Guardian review modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade and move behavior unchanged into modules for review vocabulary, policy decisions, model-review prompt/parsing, and denial breaker state. Move inline tests into integration tests by ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `novex-ai-core`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new approval-review behavior.
- Preserve root-level exports such as `GuardianReviewInput`, `review_tool_approval`, `build_guardian_model_review_prompt`, `parse_guardian_model_assessment`, and `GuardianRejectionCircuitBreaker`.
- Keep cross-crate dependency direction as `novex-approval-review -> novex-ai-core`.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-approval-review`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-approval-review/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-approval-review/src/types.rs`
  - Owns Guardian enums, request/decision DTOs, transcript/action/prompt/assessment DTOs, parse error, and `GUARDIAN_REVIEWER_NAME`.
- Create: `crates/novex-approval-review/src/policy.rs`
  - Owns policy-only review, model-assessment-to-decision mapping, and fail-closed decisions.
- Create: `crates/novex-approval-review/src/model_review.rs`
  - Owns prompt construction, model assessment parsing, JSON fence stripping, and field parsing helpers.
- Create: `crates/novex-approval-review/src/breaker.rs`
  - Owns denial breaker constants and `GuardianRejectionCircuitBreaker`.
- Modify: `crates/novex-approval-review/src/lib.rs`
  - Keep only module declarations, root re-exports, `CRATE_ID`, and `module()`.

---

### Task 1: Add Approval Review Structure Tests

**Files:**
- Create: `crates/novex-approval-review/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_approval_review`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-approval-review/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_approval_review::{
    build_guardian_model_review_prompt, guardian_review_failure_decision,
    parse_guardian_model_assessment, review_tool_approval, review_tool_approval_with_model_assessment,
    GuardianApprovalPolicy, GuardianModelAssessment, GuardianModelReviewRequest,
    GuardianRejectionCircuitBreaker, GuardianReviewFailureReason, GuardianReviewInput,
    GuardianReviewOutcome, GuardianReviewedAction, GuardianRiskLevel, GuardianTranscriptEntry,
    GuardianTranscriptRole, GuardianUserAuthorization, GUARDIAN_REVIEWER_NAME,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn review_input() -> GuardianReviewInput {
    GuardianReviewInput {
        tool_code: "github.issue.write".to_owned(),
        risk_level: GuardianRiskLevel::Medium,
        approval_policy: GuardianApprovalPolicy::OnRisk,
        user_authorization: GuardianUserAuthorization::Missing,
        auto_approved: true,
        reviewer_enabled: false,
    }
}

#[test]
fn lib_rs_is_facade_for_approval_review_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["breaker", "model_review", "policy", "types"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum GuardianRiskLevel",
        "pub struct GuardianReviewInput",
        "pub fn review_tool_approval",
        "pub fn build_guardian_model_review_prompt",
        "pub fn parse_guardian_model_assessment",
        "pub struct GuardianRejectionCircuitBreaker",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn approval_review_domain_modules_exist() {
    for module in [
        "src/breaker.rs",
        "src/model_review.rs",
        "src/policy.rs",
        "src/types.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_policy_model_review_and_breaker_contracts() {
    let decision = review_tool_approval(review_input());
    assert_eq!(decision.outcome, GuardianReviewOutcome::Approved);

    let request = GuardianModelReviewRequest {
        transcript: vec![GuardianTranscriptEntry {
            role: GuardianTranscriptRole::User,
            content: "Please create the GitHub issue".to_owned(),
        }],
        reviewed_action: GuardianReviewedAction {
            tool_code: "github.issue.write".to_owned(),
            arguments: serde_json::json!({"title":"Bug"}),
            permission_code: Some("ai:agent:run".to_owned()),
        },
        retry_reason: None,
    };
    let prompt = build_guardian_model_review_prompt(&request).unwrap();
    assert!(prompt[0].content.contains("Novex Guardian"));

    let assessment = parse_guardian_model_assessment(
        r#"{"risk_level":"medium","user_authorization":"explicit","outcome":"approved","rationale":"User asked for this."}"#,
    )
    .unwrap();
    let model_decision = review_tool_approval_with_model_assessment(review_input(), assessment);
    assert_eq!(model_decision.reviewer_name.as_deref(), Some(GUARDIAN_REVIEWER_NAME));

    let failure = guardian_review_failure_decision(
        review_input(),
        GuardianReviewFailureReason::Timeout,
        "review timed out",
    );
    assert!(!failure.can_execute);

    let mut breaker = GuardianRejectionCircuitBreaker::default();
    assert!(!breaker.record_denial());
    assert!(!breaker.record_denial());
    assert!(breaker.record_denial());
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-approval-review --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-approval-review/src/types.rs`
- Create: `crates/novex-approval-review/src/policy.rs`
- Create: `crates/novex-approval-review/src/model_review.rs`
- Create: `crates/novex-approval-review/src/breaker.rs`
- Create: `crates/novex-approval-review/tests/policy.rs`
- Create: `crates/novex-approval-review/tests/model_review.rs`
- Create: `crates/novex-approval-review/tests/breaker.rs`
- Create: `crates/novex-approval-review/tests/module_contract.rs`
- Modify: `crates/novex-approval-review/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move types**

Move all Guardian enums and DTO structs from constants through `GuardianModelReviewParseError` into `src/types.rs`. Make `GuardianTranscriptRole::as_str` `pub(crate)` so `model_review.rs` can format transcript entries.

- [ ] **Step 2: Move policy functions**

Move `review_tool_approval`, `review_tool_approval_with_model_assessment`, and `guardian_review_failure_decision` into `src/policy.rs`.

- [ ] **Step 3: Move model-review prompt and parser helpers**

Move `build_guardian_model_review_prompt`, `parse_guardian_model_assessment`, `strip_json_fence`, `parse_guardian_risk`, `parse_guardian_authorization`, `parse_guardian_outcome`, `normalized_string`, and `parse_error` into `src/model_review.rs`.

- [ ] **Step 4: Move denial breaker**

Move denial breaker constants and `GuardianRejectionCircuitBreaker` into `src/breaker.rs`.

- [ ] **Step 5: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod breaker;
mod model_review;
mod policy;
mod types;

use novex_ai_core::FoundationModule;

pub use breaker::{
    GuardianRejectionCircuitBreaker, AUTO_REVIEW_DENIAL_WINDOW_SIZE,
    MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN, MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN,
};
pub use model_review::{build_guardian_model_review_prompt, parse_guardian_model_assessment};
pub use policy::{
    guardian_review_failure_decision, review_tool_approval,
    review_tool_approval_with_model_assessment,
};
pub use types::{
    GuardianApprovalPolicy, GuardianDecisionSource, GuardianModelAssessment,
    GuardianModelReviewParseError, GuardianModelReviewRequest, GuardianPromptMessage,
    GuardianReviewDecision, GuardianReviewFailureReason, GuardianReviewInput,
    GuardianReviewOutcome, GuardianReviewStatus, GuardianReviewedAction, GuardianRiskLevel,
    GuardianTranscriptEntry, GuardianTranscriptRole, GuardianUserAuthorization,
    GUARDIAN_REVIEWER_NAME,
};

pub const CRATE_ID: &str = "novex-approval-review";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Approval Review",
        "ai-foundation",
        "Guardian approval review contracts, fail-closed policy decisions, and denial breakers.",
    )
}
```

- [ ] **Step 6: Move tests**

Use root imports in integration tests. Move policy decision tests to `tests/policy.rs`, prompt/parser/model-assessment tests to `tests/model_review.rs`, denial breaker tests to `tests/breaker.rs`, and `module_describes_approval_review_boundary` to `tests/module_contract.rs` if present. If no module test exists, keep only the structure test for this crate.

- [ ] **Step 7: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-approval-review/src/lib.rs
cargo test -p novex-approval-review
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-approval-review` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-approval-review
cargo test -p backend application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-approval-review/src crates/novex-approval-review/tests docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex approval review into focused modules"
```

Expected: commit succeeds.

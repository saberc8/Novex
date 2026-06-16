# Agent Guardian Model Review Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a model-backed Codex Guardian review path to Novex approval handling with a dedicated model route purpose, strict prompt/parser contract, fail-closed timeout/failure semantics, backend approval evidence, and eval tags.

**Architecture:** `novex-model` exposes `guardian_review` as a first-class LLM route purpose. `novex-approval-review` owns the Guardian prompt, assessment parser, and model-to-decision mapping. `backend-rust` calls the configured Guardian route when auto-approval is enabled for a tool that would otherwise pause, and `novex-eval` tags the enriched review payload.

**Tech Stack:** Rust, serde, serde_json, tokio timeout, existing ModelRuntimeService, Cargo offline tests.

---

### Task 1: Model Route Purpose

**Files:**
- Modify: `crates/novex-model/src/lib.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests proving:

- `ModelRoutePurpose::parse("guardian_review") == Some(ModelRoutePurpose::GuardianReview)`
- `ModelRoutePurpose::GuardianReview.as_str() == "guardian_review"`
- default env LLM route purposes include `GuardianReview`
- `route_target_for_purpose(ModelRoutePurpose::GuardianReview)` maps to `ModelRuntimeTarget::Llm`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-model guardian_review --offline
cargo test -p backend-rust guardian_review_model_route --offline
```

Expected: FAIL until the enum and route mappings exist.

**Step 3: Implement route purpose**

Add `GuardianReview` to model enum, parse/as_str, default LLM purposes, runtime purpose ordering, and backend route-target mapping.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p novex-model guardian_review --offline
cargo test -p backend-rust guardian_review_model_route --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-model/src/lib.rs backend/src/application/ai/model_service.rs
git commit -m "feat: add guardian review model route purpose"
```

### Task 2: Guardian Prompt And Parser Contract

**Files:**
- Modify: `crates/novex-approval-review/Cargo.toml`
- Modify: `crates/novex-approval-review/src/lib.rs`

**Step 1: Write failing tests**

Add tests for:

- prompt includes transcript boundaries, planned action JSON, and strict JSON schema instruction,
- parser accepts plain JSON and fenced JSON,
- parser rejects malformed JSON or missing rationale,
- model approved assessment maps to `source = guardian` and executable decision,
- timeout/failure decision maps to `needs_human` with `failureReason`.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-approval-review guardian_model_review --offline
```

Expected: FAIL until prompt/parser types and helpers exist.

**Step 3: Implement minimal contract**

Add `GuardianTranscriptEntry`, `GuardianReviewedAction`, `GuardianModelReviewRequest`, `GuardianPromptMessage`, `GuardianModelAssessment`, `GuardianReviewStatus`, `build_guardian_model_review_prompt`, `parse_guardian_model_assessment`, `review_tool_approval_with_model_assessment`, and `guardian_review_failure_decision`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p novex-approval-review guardian_model_review --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-approval-review/Cargo.toml crates/novex-approval-review/src/lib.rs
git commit -m "feat: add guardian model review contract"
```

### Task 3: Backend Reviewer Adapter

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests proving:

- backend source calls `chat_completion_for_purpose(ModelRoutePurpose::GuardianReview, ...)`,
- backend source wraps the reviewer call in `tokio::time::timeout`,
- transcript construction includes prior runtime items and proposed tool action,
- approval payload source contains Guardian model metadata fields.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend-rust guardian_model_review --offline
```

Expected: FAIL until backend adapter functions exist.

**Step 3: Implement adapter**

- Add `GUARDIAN_REVIEW_TIMEOUT`.
- Add transcript/action builders.
- Add async reviewer call that uses `ModelRoutePurpose::GuardianReview`.
- Parse model response into `GuardianModelAssessment`.
- Fail closed on timeout, provider error, or parse error.
- Use static review for non-auto-approve pauses.
- Serialize reviewer metadata into `guardianReview`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p backend-rust guardian_model_review --offline
cargo test -p backend-rust guardian_review --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: call guardian review model for approval pauses"
```

### Task 4: Eval Tags And Migration Matrix

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing tests**

Add eval test proving an approval-requested trace with model-backed `guardianReview` yields:

- `guardianReviewStatus`
- `guardianReviewFailureReason`
- `guardianReviewModelRouteId`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-eval guardian_model_review --offline
```

Expected: FAIL until eval extracts model reviewer tags.

**Step 3: Implement eval extraction and docs update**

- Extend Guardian review summary helper.
- Update migration matrix Guardian row and acceptance evidence.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p novex-eval guardian_model_review --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: tag guardian model review evidence"
```

### Task 5: Final Verification And Merge

**Files:**
- All modified files

**Step 1: Verify feature branch**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

**Step 2: Merge to main**

Fast-forward or merge `feat/enterprise-agent-foundation` into `main`.

**Step 3: Verify main**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

**Step 4: Report**

Summarize the Guardian model reviewer path, remaining full Codex session-manager gap, exact verification commands, and merge commit.

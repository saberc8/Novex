# Agent Route Multi-Hop Fallback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend model provider lifecycle from one-hop fallback to bounded multi-hop route fallback.

**Architecture:** Keep `ai_model_route.fallback_route_id` and `fallback_plan_for_purpose_with_route_id` as the policy source. Add small pure helpers for fallback bounds/cycle decisions, then refactor `execute_normalized_chat_completion_with_fallback` so it iterates through eligible route chains and preserves ordered `providerAttempts`.

**Tech Stack:** Rust, `backend-rust`, `novex-model`, `serde_json`, `HashSet`, existing `ModelProviderAttempt` trace/eval contract.

---

### Task 1: Commit Design And Plan

**Files:**
- Create: `docs/plans/2026-06-17-agent-route-multi-hop-fallback-design.md`
- Create: `docs/plans/2026-06-17-agent-route-multi-hop-fallback.md`

**Step 1: Review docs**

Run:

```bash
git diff -- docs/plans/2026-06-17-agent-route-multi-hop-fallback-design.md docs/plans/2026-06-17-agent-route-multi-hop-fallback.md
```

Expected: docs describe bounded route chains, cycle protection, circuit breaker interaction, and verification commands.

**Step 2: Commit**

Run:

```bash
git add docs/plans/2026-06-17-agent-route-multi-hop-fallback-design.md docs/plans/2026-06-17-agent-route-multi-hop-fallback.md
git commit -m "docs: plan multi-hop model fallback"
```

### Task 2: Add Pure Multi-Hop Chain Guards

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn multi_hop_fallback_allows_bounded_new_routes() {
    let mut visited = HashSet::from(["runtime.llm".to_owned()]);

    assert!(model_fallback_chain_can_visit(
        &visited,
        "runtime.llm.backup",
        0,
    ));
    visited.insert("runtime.llm.backup".to_owned());
    assert!(model_fallback_chain_can_visit(
        &visited,
        "runtime.llm.global",
        MAX_MODEL_FALLBACK_HOPS - 1,
    ));
}

#[test]
fn multi_hop_fallback_blocks_cycles_and_hop_overflow() {
    let visited = HashSet::from(["runtime.llm".to_owned()]);

    assert!(!model_fallback_chain_can_visit(&visited, "runtime.llm", 0));
    assert!(!model_fallback_chain_can_visit(
        &visited,
        "runtime.llm.global",
        MAX_MODEL_FALLBACK_HOPS,
    ));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust multi_hop_fallback --offline
```

Expected: FAIL because `MAX_MODEL_FALLBACK_HOPS` and `model_fallback_chain_can_visit` do not exist.

**Step 3: Implement helpers**

Add:

```rust
const MAX_MODEL_FALLBACK_HOPS: usize = 3;

fn model_fallback_chain_can_visit(
    visited_route_ids: &HashSet<String>,
    next_route_id: &str,
    fallback_hops: usize,
) -> bool {
    fallback_hops < MAX_MODEL_FALLBACK_HOPS && !visited_route_ids.contains(next_route_id)
}
```

Import `HashSet`.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust multi_hop_fallback --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: add model fallback chain guards"
```

### Task 3: Refactor Runtime Fallback Into A Route Chain

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing source-contract test**

Add:

```rust
#[test]
fn multi_hop_fallback_source_contract_iterates_route_chain() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("while fallback_hops <= MAX_MODEL_FALLBACK_HOPS"));
    assert!(source.contains("model_fallback_chain_can_visit(&visited_route_ids"));
    assert!(source.contains("fallback_plan_for_purpose_with_route_id(purpose, Some(current_route.route_id()))"));
    assert!(source.contains("attempt_kind = if fallback_hops == 0"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust multi_hop_fallback_source --offline
```

Expected: FAIL because the runtime still only executes one fallback helper.

**Step 3: Implement route-chain loop**

Refactor `execute_normalized_chat_completion_with_fallback`:

1. Use `current_route`, `visited_route_ids`, `fallback_hops`, and `attempts`.
2. Load the current route's fallback plan before attempting it.
3. If the route's circuit is open and fallback is enabled, push the skipped attempt and move to the next route.
4. Attempt the current route.
5. On success, rewrite nested provider attempts to `primary` for hop 0 and `fallback` for later hops, prepend accumulated attempts, and return.
6. On fallback-eligible error, record a failed attempt, open circuit if configured, and move to next route when allowed.
7. Stop on missing fallback, disabled fallback, cycle, hop limit, or missing route.

Keep the existing `execute_fallback_model_chat_completion` only if it is still useful; otherwise remove it with the refactor.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust multi_hop_fallback_source --offline
cargo test -p backend-rust provider_lifecycle --offline
cargo test -p backend-rust route_circuit_breaker --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: iterate model fallback route chains"
```

### Task 4: Add Trace/Eval Matrix Evidence

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Change rollout trace status from `slice-8 implemented` to `slice-9 implemented`. Update notes to include `multi-hop provider lifecycle attempts`; leave `persisted cross-process breaker state` as next.

Add the new focused command to the Rollout/trace/eval acceptance row:

```bash
cargo test -p backend-rust multi_hop_fallback --offline
```

Add this implementation plan under follow-ups.

**Step 2: Commit**

Run:

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record multi-hop fallback progress"
```

### Task 5: Final Verification And Merge

**Files:**
- All touched files.

**Step 1: Run focused verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust multi_hop_fallback --offline
cargo test -p backend-rust provider_lifecycle --offline
cargo test -p backend-rust route_circuit_breaker --offline
cargo test -p backend-rust route_circuit_breaker_trace --offline
cargo test -p novex-eval provider_fallback --offline
cargo test -p novex-eval circuit_breaker --offline
```

Expected: PASS.

**Step 2: Run full verification**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS, with `live_rag_e2e` ignored unless infra is available.

**Step 3: Merge back to local main**

Run:

```bash
git status --short --branch
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex
git status --short --branch
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation multi-hop fallback"
cargo fmt -- --check
cargo test --workspace --offline
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation
git merge --ff-only main
```

Expected: main and feature worktree end on the same merge commit with clean status.

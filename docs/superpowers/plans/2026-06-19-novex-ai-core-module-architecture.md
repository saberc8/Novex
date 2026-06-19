# Novex AI Core Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-ai-core` from a single `src/lib.rs` into focused core contract modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade and move behavior unchanged into modules for foundation module metadata, tenant/resource context, integration usage metering, run graph status/events, and task budget normalization. Move inline tests into integration tests by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `chrono`, `serde`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new core behavior.
- Preserve root-level exports such as `FoundationModule`, `TenantContext`, `IntegrationUsageSubject`, `RunStatus`, and `normalize_task_budget`.
- Keep `novex-ai-core` dependency-free from other Novex crates.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-ai-core`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-ai-core/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-ai-core/src/module.rs`
  - Owns `FoundationStatus`, `FoundationModule`, `crate_module`, and `foundation_modules`.
- Create: `crates/novex-ai-core/src/context.rs`
  - Owns `TenantContext` and `ResourceRef`.
- Create: `crates/novex-ai-core/src/integration_usage.rs`
  - Owns integration usage constants, principal/subject/window/error types, usage subject construction, window construction, and limit enforcement.
- Create: `crates/novex-ai-core/src/run_graph.rs`
  - Owns run status, transition validation, step type, pause reason, and event kind.
- Create: `crates/novex-ai-core/src/budget.rs`
  - Owns task budget DTOs, budget constants, normalization, and validation helper.
- Modify: `crates/novex-ai-core/src/lib.rs`
  - Keep only module declarations, root re-exports, and `CRATE_ID`.

---

### Task 1: Add Core Structure Tests

**Files:**
- Create: `crates/novex-ai-core/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_ai_core`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-ai-core/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use chrono::Timelike;
use novex_ai_core::{
    build_integration_usage_subject, can_transition_run_status, enforce_integration_usage_limits,
    foundation_modules, integration_usage_windows, normalize_task_budget, FoundationModule,
    FoundationStatus, IntegrationPrincipalType, IntegrationUsageLimitError, ResourceRef, RunStatus,
    TaskBudget, TenantContext, INTEGRATION_QPS_RESOURCE, INTEGRATION_USAGE_UNIT,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_ai_core_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["budget", "context", "integration_usage", "module", "run_graph"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct FoundationModule",
        "pub struct TenantContext",
        "pub struct IntegrationUsageSubject",
        "pub enum RunStatus",
        "pub struct TaskBudget",
        "pub fn normalize_task_budget",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn ai_core_domain_modules_exist() {
    for module in [
        "src/budget.rs",
        "src/context.rs",
        "src/integration_usage.rs",
        "src/module.rs",
        "src/run_graph.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_core_contracts() {
    let module = FoundationModule::skeleton("run-graph", "Run Graph", "core", "runs");
    assert_eq!(module.status, FoundationStatus::Skeleton);
    assert!(foundation_modules().iter().any(|module| module.id == "run-graph"));

    let tenant = TenantContext {
        tenant_id: "tenant-1".to_owned(),
        user_id: Some("user-1".to_owned()),
        role_ids: vec!["admin".to_owned()],
    };
    let resource = ResourceRef {
        resource_type: "dataset".to_owned(),
        resource_id: "42".to_owned(),
        tenant_id: tenant.tenant_id.clone(),
    };
    assert_eq!(resource.tenant_id, "tenant-1");

    let subject = build_integration_usage_subject(
        IntegrationPrincipalType::ApiKey,
        11,
        "42",
        2,
        5,
    )
    .unwrap();
    assert_eq!(subject.scope_type, "api_key");
    assert_eq!(
        enforce_integration_usage_limits(&subject, 3, 5).unwrap_err(),
        IntegrationUsageLimitError::QpsExceeded
    );

    let now = chrono::DateTime::parse_from_rfc3339("2026-06-06T08:09:10Z")
        .unwrap()
        .naive_utc();
    let windows = integration_usage_windows(now);
    assert_eq!(windows[0].resource_type, INTEGRATION_QPS_RESOURCE);
    assert_eq!(windows[0].usage_unit, INTEGRATION_USAGE_UNIT);
    assert_eq!(windows[0].window_start.second(), 10);

    assert!(can_transition_run_status(RunStatus::Queued, RunStatus::Running));
    assert!(RunStatus::Succeeded.is_terminal());

    let budget = normalize_task_budget(TaskBudget {
        max_steps: Some(3),
        max_tool_calls: Some(1),
        max_seconds: None,
        max_cost_cents: None,
    })
    .unwrap();
    assert_eq!(budget.max_seconds, Some(120));
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-ai-core --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-ai-core/src/module.rs`
- Create: `crates/novex-ai-core/src/context.rs`
- Create: `crates/novex-ai-core/src/integration_usage.rs`
- Create: `crates/novex-ai-core/src/run_graph.rs`
- Create: `crates/novex-ai-core/src/budget.rs`
- Create: `crates/novex-ai-core/tests/module.rs`
- Create: `crates/novex-ai-core/tests/integration_usage.rs`
- Create: `crates/novex-ai-core/tests/run_graph.rs`
- Create: `crates/novex-ai-core/tests/budget.rs`
- Modify: `crates/novex-ai-core/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move modules**

Move items according to this map:

```text
FoundationStatus, FoundationModule, crate_module, foundation_modules -> src/module.rs
TenantContext, ResourceRef -> src/context.rs
Integration usage constants/types/functions -> src/integration_usage.rs
RunStatus, transition helpers, RunStepType, PauseReason, RunEventKind -> src/run_graph.rs
TaskBudget, BudgetValidationError, budget constants, normalize_task_budget -> src/budget.rs
```

- [ ] **Step 2: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod budget;
mod context;
mod integration_usage;
mod module;
mod run_graph;

pub use budget::{
    normalize_task_budget, BudgetValidationError, TaskBudget, DEFAULT_MAX_COST_CENTS,
    DEFAULT_MAX_SECONDS, DEFAULT_MAX_STEPS, DEFAULT_MAX_TOOL_CALLS, POC_MAX_COST_CENTS,
    POC_MAX_SECONDS, POC_MAX_STEPS, POC_MAX_TOOL_CALLS,
};
pub use context::{ResourceRef, TenantContext};
pub use integration_usage::{
    build_integration_usage_subject, enforce_integration_usage_limits, integration_usage_windows,
    IntegrationPrincipalType, IntegrationUsageLimitError, IntegrationUsageSubject,
    IntegrationUsageWindow, INTEGRATION_QPS_RESOURCE, INTEGRATION_QUOTA_RESOURCE,
    INTEGRATION_USAGE_UNIT,
};
pub use module::{crate_module, foundation_modules, FoundationModule, FoundationStatus};
pub use run_graph::{
    can_transition_run_status, validate_run_transition, PauseReason, RunEventKind, RunStatus,
    RunStepType, RunTransitionError,
};

pub const CRATE_ID: &str = "novex-ai-core";
```

- [ ] **Step 3: Move tests**

Use root imports in integration tests. Move foundation module tests to `tests/module.rs`, integration usage tests to `tests/integration_usage.rs`, run graph transition tests to `tests/run_graph.rs`, and task budget tests to `tests/budget.rs`.

- [ ] **Step 4: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-ai-core/src/lib.rs
cargo test -p novex-ai-core
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-ai-core` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-ai-core
cargo test -p backend application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-ai-core/src crates/novex-ai-core/tests docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex ai core into focused modules"
```

Expected: commit succeeds.

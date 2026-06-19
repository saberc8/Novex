# Novex Agent Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-agent` from a single `src/lib.rs` into focused agent planning modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade. Move intent routing, tool selection, ReAct run planning, shared text matching, and foundation module metadata into separate files. Move inline tests into crate integration tests grouped by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `novex-ai-core`, `novex-memory`, `novex-tools`, `serde`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No agent behavior changes.
- No frontend changes.
- Preserve root-level exports such as `AgentIntent`, `AgentLoopKind`, `SelectedTool`, `AgentRunPlan`, `route_intent`, `select_tool`, `plan_react_run`, and `plan_react_run_with_memory`.
- Keep `novex-agent` dependency-free from backend crates.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-agent`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-agent/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-agent/src/intent.rs`
  - Owns `AgentIntent` and `route_intent`.
- Create: `crates/novex-agent/src/tool_selection.rs`
  - Owns `SelectedTool`, `select_tool`, and private tool policy mapping helpers.
- Create: `crates/novex-agent/src/plan.rs`
  - Owns `AgentLoopKind`, `AgentRunPlan`, `AgentPlanError`, `plan_react_run`, and `plan_react_run_with_memory`.
- Create: `crates/novex-agent/src/text.rs`
  - Owns `contains_any` as a `pub(crate)` shared helper.
- Create: `crates/novex-agent/src/module.rs`
  - Owns `module()`.
- Modify: `crates/novex-agent/src/lib.rs`
  - Keep only module declarations, root re-exports, and `CRATE_ID`.

---

### Task 1: Add Agent Structure Tests

**Files:**
- Create: `crates/novex-agent/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_agent`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-agent/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_agent::{
    module, plan_react_run, route_intent, select_tool, AgentIntent, AgentLoopKind, AgentRunPlan,
    SelectedTool,
};
use novex_ai_core::{FoundationStatus, TaskBudget};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_agent_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["intent", "module", "plan", "text", "tool_selection"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum AgentIntent",
        "pub enum AgentLoopKind",
        "pub struct SelectedTool",
        "pub struct AgentRunPlan",
        "pub fn route_intent",
        "pub fn select_tool",
        "pub fn plan_react_run",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn agent_domain_modules_exist() {
    for module in [
        "src/intent.rs",
        "src/module.rs",
        "src/plan.rs",
        "src/text.rs",
        "src/tool_selection.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_agent_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-agent");
    assert_eq!(module.status, FoundationStatus::Skeleton);

    assert_eq!(route_intent("send a Feishu reminder"), AgentIntent::ToolTask);
    let tool: SelectedTool = select_tool("read GitHub file src/lib.rs").unwrap();
    assert_eq!(tool.code, "github.repo.read");

    let plan: AgentRunPlan = plan_react_run(
        "send a Feishu reminder",
        TaskBudget {
            max_steps: Some(6),
            max_tool_calls: Some(2),
            max_seconds: Some(30),
            max_cost_cents: Some(0),
        },
    )
    .unwrap();
    assert_eq!(plan.loop_kind, AgentLoopKind::ReAct);
    assert_eq!(plan.intent, AgentIntent::ToolTask);
    assert!(plan.requires_approval);
    assert!(plan.steps.iter().any(|step| step == "action"));
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-agent --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-agent/src/intent.rs`
- Create: `crates/novex-agent/src/tool_selection.rs`
- Create: `crates/novex-agent/src/plan.rs`
- Create: `crates/novex-agent/src/text.rs`
- Create: `crates/novex-agent/src/module.rs`
- Create: `crates/novex-agent/tests/intent.rs`
- Create: `crates/novex-agent/tests/tool_selection.rs`
- Create: `crates/novex-agent/tests/plan.rs`
- Create: `crates/novex-agent/tests/module.rs`
- Modify: `crates/novex-agent/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move modules**

Move items according to this map:

```text
AgentIntent, route_intent -> src/intent.rs
SelectedTool, select_tool, selected_tool, risk_level_value -> src/tool_selection.rs
AgentLoopKind, AgentRunPlan, AgentPlanError, plan_react_run, plan_react_run_with_memory -> src/plan.rs
contains_any -> src/text.rs as pub(crate)
module -> src/module.rs
```

`intent.rs` imports `select_tool` from `crate::tool_selection` and `contains_any` from `crate::text`. `tool_selection.rs` imports `contains_any` from `crate::text`.

- [ ] **Step 2: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod intent;
mod module;
mod plan;
mod text;
mod tool_selection;

pub use intent::{route_intent, AgentIntent};
pub use module::module;
pub use plan::{plan_react_run, plan_react_run_with_memory, AgentLoopKind, AgentPlanError, AgentRunPlan};
pub use tool_selection::{select_tool, SelectedTool};

pub const CRATE_ID: &str = "novex-agent";
```

- [ ] **Step 3: Move tests**

Use root imports in integration tests. Move module metadata tests to `tests/module.rs`, intent routing tests to `tests/intent.rs`, tool selection tests to `tests/tool_selection.rs`, and plan/budget/memory tests to `tests/plan.rs`.

- [ ] **Step 4: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-agent/src/lib.rs
cargo test -p novex-agent
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-agent` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-agent
cargo test -p backend-rust application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-agent/src crates/novex-agent/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex agent into focused modules"
```

Expected: commit succeeds.

# Novex Trace Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-trace` from a single `src/lib.rs` into focused trace event and replay modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade. Move trace event vocabulary and constructors into `event.rs`, bundle ordering/count/replay logic into `bundle.rs`, replay summary DTO into `summary.rs`, and foundation module metadata into `module.rs`. Move inline tests into crate integration tests grouped by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `novex-ai-core`, `serde`, `serde_json`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No trace behavior changes.
- No frontend changes.
- Preserve root-level exports such as `TraceEventKind`, `TraceEvent`, `TraceBundle`, `TraceReplaySummary`, and `module`.
- Keep `novex-trace` dependency-free from backend crates.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-trace`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-trace/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-trace/src/event.rs`
  - Owns `TraceEventKind`, `TraceEvent`, and event constructors.
- Create: `crates/novex-trace/src/bundle.rs`
  - Owns `TraceBundle` and replay status derivation.
- Create: `crates/novex-trace/src/summary.rs`
  - Owns `TraceReplaySummary`.
- Create: `crates/novex-trace/src/module.rs`
  - Owns `module()`.
- Modify: `crates/novex-trace/src/lib.rs`
  - Keep only module declarations, root re-exports, and `CRATE_ID`.

---

### Task 1: Add Trace Structure Tests

**Files:**
- Create: `crates/novex-trace/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_trace`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-trace/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_trace::{module, TraceBundle, TraceEvent, TraceEventKind, TraceReplaySummary};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_trace_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["bundle", "event", "module", "summary"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum TraceEventKind",
        "pub struct TraceEvent",
        "pub struct TraceBundle",
        "pub struct TraceReplaySummary",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn trace_domain_modules_exist() {
    for module in [
        "src/bundle.rs",
        "src/event.rs",
        "src/module.rs",
        "src/summary.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_trace_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-trace");
    assert_eq!(module.status, FoundationStatus::Skeleton);

    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::tool_call(2, "call-1", "rag.search"))
        .with_event(TraceEvent::final_answer(3, "done"));
    assert_eq!(bundle.events[0].kind, TraceEventKind::ToolCall);
    assert_eq!(bundle.tool_call_count(), 1);

    let summary: TraceReplaySummary = bundle.replay_summary();
    assert_eq!(summary.trace_id, "agent-1");
    assert_eq!(summary.final_status, "succeeded");
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-trace --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-trace/src/event.rs`
- Create: `crates/novex-trace/src/bundle.rs`
- Create: `crates/novex-trace/src/summary.rs`
- Create: `crates/novex-trace/src/module.rs`
- Create: `crates/novex-trace/tests/bundle.rs`
- Create: `crates/novex-trace/tests/event.rs`
- Create: `crates/novex-trace/tests/module.rs`
- Modify: `crates/novex-trace/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move modules**

Move items according to this map:

```text
TraceEventKind, TraceEvent -> src/event.rs
TraceBundle -> src/bundle.rs
TraceReplaySummary -> src/summary.rs
module -> src/module.rs
```

- [ ] **Step 2: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod bundle;
mod event;
mod module;
mod summary;

pub use bundle::TraceBundle;
pub use event::{TraceEvent, TraceEventKind};
pub use module::module;
pub use summary::TraceReplaySummary;

pub const CRATE_ID: &str = "novex-trace";
```

- [ ] **Step 3: Move tests**

Use root imports in integration tests. Move bundle ordering/runtime tests to `tests/bundle.rs`, inference event constructor tests to `tests/event.rs`, and add module metadata coverage to `tests/module.rs`.

- [ ] **Step 4: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-trace/src/lib.rs
cargo test -p novex-trace
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-trace` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-trace
cargo test -p backend-rust application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-trace/src crates/novex-trace/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex trace into focused modules"
```

Expected: commit succeeds.

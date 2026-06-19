# Novex Small Crates Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize the remaining non-facade small crates, `novex-agent-protocol` and `novex-memory`, so `src/lib.rs` is a facade and inline tests move to integration tests.

**Architecture:** Split `novex-agent-protocol` into turn item and outcome modules. Split `novex-memory` into memory type, context building, and foundation metadata modules. Preserve crate-root public APIs.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `novex-ai-core`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No behavior changes.
- No frontend changes.
- Preserve all existing crate-root exports.
- Keep both crates dependency-free from backend crates.
- Run `cargo fmt --all -- --check`, focused crate tests, and `git diff --check` before considering this slice complete.

---

## File Structure

- `crates/novex-agent-protocol/src/item.rs`: `AgentTurnItemType`, `ToolObservationStatus`, `AgentTurnItem`.
- `crates/novex-agent-protocol/src/outcome.rs`: `TurnOutcome`.
- `crates/novex-agent-protocol/src/lib.rs`: facade.
- `crates/novex-agent-protocol/tests/module_structure.rs`, `item.rs`, `outcome.rs`.
- `crates/novex-memory/src/types.rs`: `MemoryScope`, `MemoryWritePolicy`, `MemoryScopeRef`, `MemorySnippet`, `MemoryAccessContext`, `MemoryContext`.
- `crates/novex-memory/src/context.rs`: `build_memory_context`.
- `crates/novex-memory/src/module.rs`: `module()`.
- `crates/novex-memory/src/lib.rs`: facade.
- `crates/novex-memory/tests/module_structure.rs`, `context.rs`, `module.rs`.

---

### Task 1: Split `novex-agent-protocol`

**Files:**
- Create: `crates/novex-agent-protocol/tests/module_structure.rs`
- Create: `crates/novex-agent-protocol/src/item.rs`
- Create: `crates/novex-agent-protocol/src/outcome.rs`
- Create: `crates/novex-agent-protocol/tests/item.rs`
- Create: `crates/novex-agent-protocol/tests/outcome.rs`
- Modify: `crates/novex-agent-protocol/src/lib.rs`

**Interfaces:**
- Consumes: existing crate-root protocol API.
- Produces: same root exports through a facade.

- [ ] **Step 1: Write failing module structure test**

Assert `src/lib.rs` declares `mod item;` and `mod outcome;`, does not contain `pub enum AgentTurnItem`, `pub enum TurnOutcome`, or `#[cfg(test)] mod tests`, and root APIs can construct `AgentTurnItem::tool_call`, read `call_id()`, and call `TurnOutcome::Final.is_terminal()`.

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test -p novex-agent-protocol --test module_structure
```

Expected: FAIL because module files do not exist yet.

- [ ] **Step 3: Split implementation**

Use this facade:

```rust
mod item;
mod outcome;

pub use item::{AgentTurnItem, AgentTurnItemType, ToolObservationStatus};
pub use outcome::TurnOutcome;

pub const CRATE_ID: &str = "novex-agent-protocol";
```

- [ ] **Step 4: Move tests and verify**

Move turn item serialization/call-id tests to `tests/item.rs`; move terminal outcome tests to `tests/outcome.rs`.

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-agent-protocol/src/lib.rs
cargo test -p novex-agent-protocol
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 2: Split `novex-memory`

**Files:**
- Create: `crates/novex-memory/tests/module_structure.rs`
- Create: `crates/novex-memory/src/types.rs`
- Create: `crates/novex-memory/src/context.rs`
- Create: `crates/novex-memory/src/module.rs`
- Create: `crates/novex-memory/tests/context.rs`
- Create: `crates/novex-memory/tests/module.rs`
- Modify: `crates/novex-memory/src/lib.rs`

**Interfaces:**
- Consumes: existing crate-root memory API.
- Produces: same root exports through a facade.

- [ ] **Step 1: Write failing module structure test**

Assert `src/lib.rs` declares `mod context;`, `mod module;`, and `mod types;`, does not contain `pub enum MemoryScope`, `pub struct MemoryContext`, `pub fn build_memory_context`, or inline tests, and root APIs can build a filtered `MemoryContext`.

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test -p novex-memory --test module_structure
```

Expected: FAIL because module files do not exist yet.

- [ ] **Step 3: Split implementation**

Use this facade:

```rust
mod context;
mod module;
mod types;

pub use context::build_memory_context;
pub use module::module;
pub use types::{
    MemoryAccessContext, MemoryContext, MemoryScope, MemoryScopeRef, MemorySnippet,
    MemoryWritePolicy,
};

pub const CRATE_ID: &str = "novex-memory";
```

- [ ] **Step 4: Move tests and verify**

Move module metadata tests to `tests/module.rs` and context filtering tests to `tests/context.rs`.

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-memory/src/lib.rs
cargo test -p novex-memory
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed small crate splits.
- Produces: committed, verified small crate cleanup slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-agent-protocol
cargo test -p novex-memory
cargo test -p novex-agent
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-agent-protocol/src crates/novex-agent-protocol/tests crates/novex-memory/src crates/novex-memory/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex small crates into focused modules"
```

Expected: commit succeeds.

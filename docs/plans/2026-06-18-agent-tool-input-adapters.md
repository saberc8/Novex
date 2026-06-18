# Agent Tool Input Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move pure Agent tool JSON input normalization out of `backend/src/application/ai/agent_service.rs` into `crates/novex-tools`, while preserving existing Feishu, media image, and GitHub execution behavior.

**Architecture:** `novex-tools` owns the model-visible tool contract and should also own the pure JSON-to-request adapter for built-in Agent tools. Connector-specific request DTOs stay in `novex-connectors`, and backend remains responsible for credentials, route resolution, HTTP calls, persistence, and auditing.

**Tech Stack:** Rust workspace crates, `serde_json::Value`, existing `novex-connectors` GitHub/Feishu DTOs, existing `novex-tools::MediaImageGenerationRequest`.

## Global Constraints

- Do not change external tool response payload shape, audit payload shape, status strings, or final output text.
- Do not move live HTTP, model route resolution, database writes, or credential lookup in this slice.
- Keep legacy natural-language parsing for GitHub repo/search/read inputs.
- Keep backend tests as compatibility/source-contract coverage while moving behavior tests into `novex-tools`.
- Use TDD: add failing tests before production code, verify RED, implement minimally, verify GREEN.

---

### Task 1: Shared Tool Input Adapter API

**Files:**
- Modify: `crates/novex-tools/src/lib.rs`
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Produces: `feishu_message_text_from_tool_input(input: &serde_json::Value) -> String`
- Produces: `media_image_request_from_tool_input(input: &serde_json::Value) -> MediaImageGenerationRequest`
- Produces: `github_search_request_from_tool_input(input: &serde_json::Value) -> Option<novex_connectors::GitHubCodeSearchRequest>`
- Produces: `github_read_request_from_tool_input(input: &serde_json::Value) -> Option<novex_connectors::GitHubFileReadRequest>`
- Consumes: `novex_connectors::{GitHubCodeSearchRequest, GitHubFileReadRequest}` and `MediaImageGenerationRequest`

- [ ] **Step 1: Write failing `novex-tools` behavior tests**

Add tests in `crates/novex-tools/src/lib.rs` that call the four new public adapter functions. Cover explicit Feishu message priority, media prompt/size/count, GitHub explicit search/read input, and GitHub natural-language search/read input.

- [ ] **Step 2: Run RED**

Run: `cargo test -p novex-tools agent_tool_input --offline`

Expected: fail because the adapter functions are not defined in `novex-tools`.

- [ ] **Step 3: Write failing backend source-contract test**

Update the backend source-contract around tool execution so it expects imports/calls from `novex-tools` and no local helper definitions for the four adapter functions.

- [ ] **Step 4: Run RED**

Run: `cargo test -p backend-rust agent_tool_input_adapters_live_in_novex_tools --offline`

Expected: fail because `agent_service.rs` still defines and calls local adapter functions.

- [ ] **Step 5: Implement shared adapter functions**

Move the pure helper logic into `crates/novex-tools/src/lib.rs`, keeping helper functions private:

```rust
pub fn feishu_message_text_from_tool_input(input: &Value) -> String
pub fn media_image_request_from_tool_input(input: &Value) -> MediaImageGenerationRequest
pub fn github_search_request_from_tool_input(input: &Value) -> Option<GitHubCodeSearchRequest>
pub fn github_read_request_from_tool_input(input: &Value) -> Option<GitHubFileReadRequest>
```

- [ ] **Step 6: Consume shared functions in backend**

Import the four functions from `novex_tools` in `backend/src/application/ai/agent_service.rs`, delete the local duplicate definitions and their private helper functions when they are no longer used.

- [ ] **Step 7: Update matrix**

Update the Tool router row and acceptance evidence in `docs/plans/2026-06-16-codex-migration-matrix.md` to record this as the next concrete executor extraction slice.

- [ ] **Step 8: Verify GREEN**

Run:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p novex-tools agent_tool_input --offline
cargo test -p backend-rust agent_tool_input_adapters_live_in_novex_tools --offline
cargo test -p backend-rust github_search_request_from_tool_input --offline
cargo test -p backend-rust media_image_request_from_tool_input --offline
cargo test -p backend-rust feishu_message_text --offline
cargo test --workspace --offline
```

- [ ] **Step 9: Commit, merge, clean**

Commit plan and implementation separately, fast-forward merge `feat/enterprise-agent-foundation` into `main`, rerun the same verification on main, then run `cargo clean` in both the main worktree and `.worktrees/enterprise-agent-foundation`.

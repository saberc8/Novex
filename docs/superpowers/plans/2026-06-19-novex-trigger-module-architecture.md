# Novex Trigger Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-trigger` from a single `src/lib.rs` into focused trigger routing modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade. Move trigger source/target vocabulary into `types.rs`, delivery routing and retry planning into `delivery.rs`, webhook signature/idempotency helpers into `webhook.rs`, and foundation module metadata into `module.rs`. Move inline tests into crate integration tests grouped by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `novex-ai-core`, `serde`, `serde_json`, `hmac`, `sha2`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No trigger behavior changes.
- No frontend changes.
- Preserve root-level exports such as `TriggerSourceKind`, `TriggerTargetKind`, `TriggerRetryPolicy`, `TriggerDeliveryInput`, `plan_trigger_delivery`, `webhook_signature`, `verify_webhook_signature`, and `normalize_idempotency_key`.
- Keep `novex-trigger` dependency-free from backend crates.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-trigger`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-trigger/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-trigger/src/types.rs`
  - Owns `TriggerSourceKind` and `TriggerTargetKind`.
- Create: `crates/novex-trigger/src/delivery.rs`
  - Owns delivery status constants, target support checks, retry policy, delivery input/plan DTOs, and delivery planning.
- Create: `crates/novex-trigger/src/webhook.rs`
  - Owns webhook signature prefix, idempotency validation, webhook signature verification, and private hex helpers.
- Create: `crates/novex-trigger/src/module.rs`
  - Owns `module()`.
- Modify: `crates/novex-trigger/src/lib.rs`
  - Keep only module declarations, root re-exports, and `CRATE_ID`.

---

### Task 1: Add Trigger Structure Tests

**Files:**
- Create: `crates/novex-trigger/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_trigger`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-trigger/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_trigger::{
    is_supported_target_kind, module, normalize_idempotency_key, plan_trigger_delivery,
    verify_webhook_signature, webhook_signature, TriggerDeliveryInput, TriggerRetryPolicy,
    TriggerSourceKind, ACCEPTED_DELIVERY_STATUS, WEBHOOK_SIGNATURE_PREFIX,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_trigger_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["delivery", "module", "types", "webhook"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum TriggerSourceKind",
        "pub struct TriggerRetryPolicy",
        "pub fn plan_trigger_delivery",
        "pub fn webhook_signature",
        "pub fn normalize_idempotency_key",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn trigger_domain_modules_exist() {
    for module in [
        "src/delivery.rs",
        "src/module.rs",
        "src/types.rs",
        "src/webhook.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_trigger_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-trigger");
    assert_eq!(module.status, FoundationStatus::Skeleton);
    assert_eq!(
        serde_json::to_value(TriggerSourceKind::PluginEvent).unwrap(),
        serde_json::json!("plugin_event")
    );

    assert!(is_supported_target_kind("agent_run"));
    let plan = plan_trigger_delivery(TriggerDeliveryInput {
        trigger_id: 7,
        trigger_code: "webhook.training.event".to_owned(),
        target_kind: "agent_run".to_owned(),
        route_config: serde_json::json!({"agentCode":"training-assistant"}),
        event_id: 11,
        retry_policy: TriggerRetryPolicy::default(),
    });
    assert_eq!(plan.status, ACCEPTED_DELIVERY_STATUS);
    assert_eq!(plan.trace_id, Some(11));

    let signature = webhook_signature("top-secret", br#"{"event":"x"}"#);
    assert!(signature.starts_with(WEBHOOK_SIGNATURE_PREFIX));
    assert!(verify_webhook_signature(
        "top-secret",
        br#"{"event":"x"}"#,
        &signature
    ));
    assert_eq!(normalize_idempotency_key(" tenant:event ").unwrap(), "tenant:event");
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-trigger --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-trigger/src/types.rs`
- Create: `crates/novex-trigger/src/delivery.rs`
- Create: `crates/novex-trigger/src/webhook.rs`
- Create: `crates/novex-trigger/src/module.rs`
- Create: `crates/novex-trigger/tests/module.rs`
- Create: `crates/novex-trigger/tests/delivery.rs`
- Create: `crates/novex-trigger/tests/webhook.rs`
- Modify: `crates/novex-trigger/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move modules**

Move items according to this map:

```text
TriggerSourceKind, TriggerTargetKind -> src/types.rs
ACCEPTED_DELIVERY_STATUS, DEAD_LETTER_DELIVERY_STATUS, is_supported_target_kind,
TriggerRetryPolicy, TriggerDeliveryInput, TriggerDeliveryPlan, plan_trigger_delivery -> src/delivery.rs
WEBHOOK_SIGNATURE_PREFIX, MAX_IDEMPOTENCY_KEY_CHARS, HmacSha256, TriggerValidationError,
webhook_signature, verify_webhook_signature, normalize_idempotency_key,
hex_encode, hex_decode, hex_nibble -> src/webhook.rs
module -> src/module.rs
```

- [ ] **Step 2: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod delivery;
mod module;
mod types;
mod webhook;

pub use delivery::{
    is_supported_target_kind, plan_trigger_delivery, TriggerDeliveryInput, TriggerDeliveryPlan,
    TriggerRetryPolicy, ACCEPTED_DELIVERY_STATUS, DEAD_LETTER_DELIVERY_STATUS,
};
pub use module::module;
pub use types::{TriggerSourceKind, TriggerTargetKind};
pub use webhook::{
    normalize_idempotency_key, verify_webhook_signature, webhook_signature,
    TriggerValidationError, WEBHOOK_SIGNATURE_PREFIX,
};

pub const CRATE_ID: &str = "novex-trigger";
```

- [ ] **Step 3: Move tests**

Use root imports in integration tests. Move module metadata tests to `tests/module.rs`, webhook/idempotency tests to `tests/webhook.rs`, and delivery routing tests to `tests/delivery.rs`.

- [ ] **Step 4: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-trigger/src/lib.rs
cargo test -p novex-trigger
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-trigger` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-trigger
cargo test -p backend-rust application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-trigger/src crates/novex-trigger/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex trigger into focused modules"
```

Expected: commit succeeds.

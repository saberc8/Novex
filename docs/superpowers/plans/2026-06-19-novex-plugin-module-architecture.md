# Novex Plugin Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-plugin` from a single `src/lib.rs` into focused plugin manifest modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade. Move plugin manifest DTOs and error vocabulary into `types.rs`, manifest validation into `validation.rs`, built-in plugin catalog construction into `builtin.rs`, and foundation module metadata into `module.rs`. Move inline tests into crate integration tests grouped by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `novex-ai-core`, `serde`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No plugin behavior changes.
- No frontend changes.
- Preserve root-level exports such as `PluginRuntime`, `PluginManifest`, `PluginManifestError`, `validate_plugin_manifest`, `required_plugin_permissions`, and `builtin_plugin_manifest`.
- Keep `novex-plugin` dependency-free from backend crates.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-plugin`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-plugin/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-plugin/src/types.rs`
  - Owns `PluginRuntime`, `PluginCapabilityKind`, `PluginCapability`, `PluginNetworkPolicy`, `PluginManifest`, and `PluginManifestError`.
- Create: `crates/novex-plugin/src/validation.rs`
  - Owns `validate_plugin_manifest`, `required_plugin_permissions`, and `ensure_non_empty`.
- Create: `crates/novex-plugin/src/builtin.rs`
  - Owns `builtin_plugin_manifest` and private capability/permission constructor helpers.
- Create: `crates/novex-plugin/src/module.rs`
  - Owns `module()`.
- Modify: `crates/novex-plugin/src/lib.rs`
  - Keep only module declarations, root re-exports, and `CRATE_ID`.

---

### Task 1: Add Plugin Structure Tests

**Files:**
- Create: `crates/novex-plugin/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_plugin`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-plugin/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_plugin::{
    builtin_plugin_manifest, module, required_plugin_permissions, validate_plugin_manifest,
    PluginCapability, PluginCapabilityKind, PluginManifest, PluginManifestError,
    PluginNetworkPolicy, PluginRuntime,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_plugin_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["builtin", "module", "types", "validation"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum PluginRuntime",
        "pub struct PluginManifest",
        "pub enum PluginManifestError",
        "pub fn validate_plugin_manifest",
        "pub fn builtin_plugin_manifest",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn plugin_domain_modules_exist() {
    for module in [
        "src/builtin.rs",
        "src/module.rs",
        "src/types.rs",
        "src/validation.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_plugin_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-plugin");
    assert_eq!(module.status, FoundationStatus::Skeleton);
    assert_eq!(PluginRuntime::HostedHttp.as_str(), "hosted_http");

    let manifest = PluginManifest {
        code: "external.webhook-tool".to_owned(),
        name: "External Webhook Tool".to_owned(),
        version: "0.1.0".to_owned(),
        runtime: PluginRuntime::HostedHttp,
        capabilities: vec![PluginCapability {
            kind: PluginCapabilityKind::Tool,
            code: "external.webhook.post".to_owned(),
            permission_code: "ai:tool:dryRun".to_owned(),
        }],
        permission_grants: vec!["ai:tool:dryRun".to_owned()],
        network: PluginNetworkPolicy {
            allowlist: vec!["api.example.com".to_owned()],
        },
    };
    validate_plugin_manifest(&manifest).unwrap();
    assert_eq!(
        required_plugin_permissions(&manifest),
        vec!["ai:tool:dryRun".to_owned()]
    );

    let builtin = builtin_plugin_manifest("builtin.github-basic").unwrap();
    assert_eq!(builtin.runtime, PluginRuntime::BuiltinAdapter);

    let mut invalid = manifest;
    invalid.permission_grants.clear();
    assert_eq!(
        validate_plugin_manifest(&invalid).unwrap_err(),
        PluginManifestError::PermissionNotGranted {
            capability_code: "external.webhook.post".to_owned(),
            permission_code: "ai:tool:dryRun".to_owned(),
        }
    );
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-plugin --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-plugin/src/types.rs`
- Create: `crates/novex-plugin/src/validation.rs`
- Create: `crates/novex-plugin/src/builtin.rs`
- Create: `crates/novex-plugin/src/module.rs`
- Create: `crates/novex-plugin/tests/module.rs`
- Create: `crates/novex-plugin/tests/validation.rs`
- Create: `crates/novex-plugin/tests/builtin.rs`
- Modify: `crates/novex-plugin/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move modules**

Move items according to this map:

```text
PluginRuntime, PluginCapabilityKind, PluginCapability, PluginNetworkPolicy,
PluginManifest, PluginManifestError -> src/types.rs
validate_plugin_manifest, required_plugin_permissions, ensure_non_empty -> src/validation.rs
builtin_plugin_manifest, skill, tool, connector, trigger, capability, permissions -> src/builtin.rs
module -> src/module.rs
```

- [ ] **Step 2: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod builtin;
mod module;
mod types;
mod validation;

pub use builtin::builtin_plugin_manifest;
pub use module::module;
pub use types::{
    PluginCapability, PluginCapabilityKind, PluginManifest, PluginManifestError,
    PluginNetworkPolicy, PluginRuntime,
};
pub use validation::{required_plugin_permissions, validate_plugin_manifest};

pub const CRATE_ID: &str = "novex-plugin";
```

- [ ] **Step 3: Move tests**

Use root imports in integration tests. Move module metadata tests to `tests/module.rs`, manifest validation tests to `tests/validation.rs`, and add built-in catalog coverage to `tests/builtin.rs`.

- [ ] **Step 4: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-plugin/src/lib.rs
cargo test -p novex-plugin
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-plugin` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-plugin
cargo test -p backend application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-plugin/src crates/novex-plugin/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex plugin into focused modules"
```

Expected: commit succeeds.

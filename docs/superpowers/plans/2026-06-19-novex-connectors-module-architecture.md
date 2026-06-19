# Novex Connectors Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-connectors` from a single `src/lib.rs` into focused connector contract modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade. Move connector kind vocabulary, credential resolution, Feishu webhook payloads, GitHub request/response helpers, and foundation module metadata into separate files. Move inline tests into crate integration tests grouped by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `novex-ai-core`, `serde`, `serde_json`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No connector behavior changes.
- No frontend changes.
- Preserve root-level exports such as `ConnectorKind`, `ConnectorCredentialBinding`, `FeishuTextMessage`, `GitHubCodeSearchRequest`, `parse_github_code_search_response`, and `select_connector_credential`.
- Keep `novex-connectors` dependency-free from backend crates.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-connectors`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-connectors/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-connectors/src/kind.rs`
  - Owns `ConnectorKind`.
- Create: `crates/novex-connectors/src/credential.rs`
  - Owns credential scope/source/binding/selection types and helpers.
- Create: `crates/novex-connectors/src/feishu.rs`
  - Owns `FeishuTextMessage`.
- Create: `crates/novex-connectors/src/github.rs`
  - Owns GitHub code search/read request DTOs, response item DTO, parser, and GitHub private parsing helpers.
- Create: `crates/novex-connectors/src/module.rs`
  - Owns `module()`.
- Modify: `crates/novex-connectors/src/lib.rs`
  - Keep only module declarations, root re-exports, and `CRATE_ID`.

---

### Task 1: Add Connector Structure Tests

**Files:**
- Create: `crates/novex-connectors/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_connectors`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-connectors/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_connectors::{
    credential_scope_code, module, parse_credential_scope, parse_github_code_search_response,
    select_connector_credential, ConnectorCredentialBinding, ConnectorCredentialSource,
    ConnectorKind, CredentialScope, FeishuTextMessage, GitHubCodeSearchRequest,
    GitHubFileReadRequest,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_connector_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["credential", "feishu", "github", "kind", "module"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum ConnectorKind",
        "pub struct ConnectorCredentialBinding",
        "pub struct FeishuTextMessage",
        "pub struct GitHubCodeSearchRequest",
        "pub fn parse_github_code_search_response",
        "pub fn select_connector_credential",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn connector_domain_modules_exist() {
    for module in [
        "src/credential.rs",
        "src/feishu.rs",
        "src/github.rs",
        "src/kind.rs",
        "src/module.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_connector_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-connectors");
    assert_eq!(module.status, FoundationStatus::Skeleton);

    let kind = ConnectorKind::GitHub;
    assert_eq!(serde_json::to_value(kind).unwrap(), serde_json::json!("git_hub"));

    let binding = ConnectorCredentialBinding {
        connector_code: "github.default".to_owned(),
        scope: CredentialScope::Tenant,
        scope_id: "1".to_owned(),
        auth_type: "oauth_app".to_owned(),
        secret_ref: "env:GITHUB_TOKEN".to_owned(),
        scopes: vec!["repo".to_owned()],
    };
    let credential = select_connector_credential(Some(&binding), &["FALLBACK"], |key| match key {
        "GITHUB_TOKEN" => Some(" token ".to_owned()),
        _ => None,
    })
    .unwrap();
    assert_eq!(credential.token, "token");
    assert_eq!(credential.source, ConnectorCredentialSource::Binding);
    assert_eq!(credential_scope_code(CredentialScope::Tenant), "tenant");
    assert_eq!(parse_credential_scope("tenant"), Some(CredentialScope::Tenant));

    let message = FeishuTextMessage::new(" hello ");
    assert_eq!(message.text, "hello");
    assert_eq!(
        message.to_webhook_payload()["content"]["text"],
        serde_json::json!("hello")
    );

    let search = GitHubCodeSearchRequest::new("acme/app", "parser")
        .with_path("/src")
        .with_limit(999);
    assert_eq!(search.query_pairs()[1], ("per_page".to_owned(), "100".to_owned()));

    let read = GitHubFileReadRequest::new("acme/app", "/src/../lib.rs").with_ref("main");
    assert_eq!(read.path, "src/lib.rs");
    assert_eq!(
        read.query_pairs(),
        vec![("ref".to_owned(), "main".to_owned())]
    );

    let items = parse_github_code_search_response(&serde_json::json!({
        "items": [{
            "path": "src/lib.rs",
            "repository": { "full_name": "acme/app" }
        }]
    }));
    assert_eq!(items[0].repository, "acme/app");
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-connectors --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-connectors/src/kind.rs`
- Create: `crates/novex-connectors/src/credential.rs`
- Create: `crates/novex-connectors/src/feishu.rs`
- Create: `crates/novex-connectors/src/github.rs`
- Create: `crates/novex-connectors/src/module.rs`
- Create: `crates/novex-connectors/tests/credential.rs`
- Create: `crates/novex-connectors/tests/feishu.rs`
- Create: `crates/novex-connectors/tests/github.rs`
- Create: `crates/novex-connectors/tests/module.rs`
- Modify: `crates/novex-connectors/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move modules**

Move items according to this map:

```text
ConnectorKind -> src/kind.rs
CredentialScope, ConnectorCredentialBinding, ConnectorCredentialSource, ResolvedConnectorCredential,
parse_credential_scope, credential_scope_code, select_connector_credential, resolve_env_secret_ref,
trim_non_empty_owned -> src/credential.rs
FeishuTextMessage -> src/feishu.rs
GitHubCodeSearchRequest, GitHubFileReadRequest, GitHubCodeSearchItem,
parse_github_code_search_response, normalize_github_path,
github_code_search_item_from_value, json_f32 -> src/github.rs
module -> src/module.rs
```

- [ ] **Step 2: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod credential;
mod feishu;
mod github;
mod kind;
mod module;

pub use credential::{
    credential_scope_code, parse_credential_scope, resolve_env_secret_ref,
    select_connector_credential, ConnectorCredentialBinding, ConnectorCredentialSource,
    CredentialScope, ResolvedConnectorCredential,
};
pub use feishu::FeishuTextMessage;
pub use github::{
    parse_github_code_search_response, GitHubCodeSearchItem, GitHubCodeSearchRequest,
    GitHubFileReadRequest,
};
pub use kind::ConnectorKind;
pub use module::module;

pub const CRATE_ID: &str = "novex-connectors";
```

- [ ] **Step 3: Move tests**

Use root imports in integration tests. Move module metadata tests to `tests/module.rs`, Feishu tests to `tests/feishu.rs`, GitHub tests to `tests/github.rs`, and credential tests to `tests/credential.rs`.

- [ ] **Step 4: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-connectors/src/lib.rs
cargo test -p novex-connectors
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-connectors` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-connectors
cargo test -p backend-rust application::ai::foundation_service::tests::summary_lists_required_foundation_crates
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-connectors/src crates/novex-connectors/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex connectors into focused modules"
```

Expected: commit succeeds.

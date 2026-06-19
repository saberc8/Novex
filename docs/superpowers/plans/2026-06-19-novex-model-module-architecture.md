# Novex Model Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-model` from a 1,214-line `src/lib.rs` into focused model-foundation modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as the crate facade and move behavior unchanged into modules for model taxonomy, runtime route/configuration, provider-neutral DTOs, usage accounting, cost estimation, route policy, and shared parsing helpers. Add integration-level structure tests that prove `lib.rs` is a facade and existing root imports keep working for backend, RAG, provider-client, and tools consumers.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `novex-ai-core`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new model behavior.
- Preserve root-level exports such as `novex_model::ModelRuntimeConfig`, `novex_model::ModelRoutePurpose`, `novex_model::ModelTokenUsage`, `novex_model::evaluate_model_route_policy`, and `novex_model::mask_api_key`.
- Keep cross-crate dependency direction as `novex-model -> novex-ai-core`.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-model`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-model/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-model/src/taxonomy.rs`
  - Owns `ModelKind`, `ModelProviderType`, `ModelRoutePurpose`, and `ModelRuntimeTarget`.
- Create: `crates/novex-model/src/route.rs`
  - Owns `ModelRuntimeRoute`, `ModelRuntimeConfig`, `ModelRuntimeSummary`, `ModelRuntimeRouteSummary`, route summary helpers, and environment route construction helpers.
- Create: `crates/novex-model/src/provider.rs`
  - Owns provider-neutral stream chunk, image generation response, rerank score, and embedding vector DTOs.
- Create: `crates/novex-model/src/usage.rs`
  - Owns `ModelTokenUsage`, `ModelTokenUsageCounts`, usage normalization, and text token estimation.
- Create: `crates/novex-model/src/cost.rs`
  - Owns `ModelUsageCostInput` and `estimate_model_cost_cents`.
- Create: `crates/novex-model/src/policy.rs`
  - Owns `ModelRoutePolicyInput`, `ModelRoutePolicyStatus`, and route policy evaluation.
- Create: `crates/novex-model/src/util.rs`
  - Owns shared JSON field parsing, registry token normalization, URL joining/normalization, non-negative numeric conversion, and public `mask_api_key`.
- Modify: `crates/novex-model/src/lib.rs`
  - Keep only module declarations, root re-exports, `CRATE_ID`, and `module()`.

---

### Task 1: Add Model Structure and Public-Facade Characterization Tests

**Files:**
- Create: `crates/novex-model/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_model`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-model/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_model::{
    estimate_model_cost_cents, estimate_model_text_tokens, evaluate_model_route_policy,
    mask_api_key, normalize_model_provider_usage, ModelEmbeddingVector, ModelKind,
    ModelMediaImageGenerationResp, ModelProviderStreamChunk, ModelProviderType,
    ModelRerankScore, ModelRoutePolicyInput, ModelRoutePurpose, ModelRuntimeConfig,
    ModelRuntimeRoute, ModelRuntimeTarget, ModelTokenUsage, ModelUsageCostInput,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_model_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "cost",
        "policy",
        "provider",
        "route",
        "taxonomy",
        "usage",
        "util",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum ModelKind",
        "pub struct ModelRuntimeConfig",
        "pub struct ModelTokenUsage",
        "pub fn normalize_model_provider_usage",
        "pub fn evaluate_model_route_policy",
        "pub fn mask_api_key",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn model_domain_modules_exist() {
    for module in [
        "src/cost.rs",
        "src/policy.rs",
        "src/provider.rs",
        "src/route.rs",
        "src/taxonomy.rs",
        "src/usage.rs",
        "src/util.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_taxonomy_and_route_contracts() {
    assert_eq!(ModelKind::parse("media_generation"), Some(ModelKind::MediaGeneration));
    assert_eq!(
        ModelProviderType::parse("openai-compatible"),
        Some(ModelProviderType::OpenAiCompatible)
    );
    assert_eq!(
        ModelRoutePurpose::parse("guardian_review"),
        Some(ModelRoutePurpose::GuardianReview)
    );
    assert_eq!(ModelRuntimeTarget::parse("rerank"), Some(ModelRuntimeTarget::Reranker));

    let route = ModelRuntimeRoute::new(
        "tenant42.rag_answer",
        ModelRuntimeTarget::Llm,
        ModelKind::Llm,
        ModelProviderType::OpenAiCompatible,
        Some("qwen-private".to_owned()),
        "https://llm.internal/v1",
        "https://llm.internal/v1/chat/completions",
        "sk-fake-private-secret-0001",
        vec![ModelRoutePurpose::RagAnswer],
        vec!["LLM_PRIVATE_KEY".to_owned()],
    )
    .unwrap();

    let summary = route.summary();
    assert_eq!(summary.route_id, "tenant42.rag_answer");
    assert_eq!(summary.masked_api_key, "sk-****0001");
    assert_eq!(mask_api_key("sk-fake-private-secret-0001"), "sk-****0001");

    let config = ModelRuntimeConfig::from_env_map(|key| {
        (key == "LLM_API_KEY").then(|| "sk-fake-llm-secret-508d".to_owned())
    });
    assert!(config.routes().is_empty());
    assert!(config.missing_env().contains(&"LLM_BASE_URL".to_owned()));
}

#[test]
fn root_facade_preserves_usage_cost_policy_and_provider_contracts() {
    let usage = normalize_model_provider_usage(&serde_json::json!({
        "usage": {"input_tokens": "11", "outputTokens": 7}
    }));
    assert_eq!(usage.accounting_counts().total_tokens, 18);
    assert_eq!(estimate_model_text_tokens("hello world"), 2);
    assert_eq!(ModelTokenUsage::default().accounting_counts().total_tokens, 0);

    let cost_cents = estimate_model_cost_cents(
        &serde_json::json!({
            "unit": "token",
            "promptCentsPer1kTokens": 0.2,
            "completionCentsPer1kTokens": 0.8,
            "requestCents": 0.05
        }),
        &ModelUsageCostInput {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
            request_count: 1,
            vector_count: 0,
        },
    );
    assert!((cost_cents - 0.65).abs() < 0.000_001);

    let status = evaluate_model_route_policy(ModelRoutePolicyInput {
        network_zone: "private",
        fallback_network_zone: Some("public"),
        fallback_policy: &serde_json::json!({"enabled": true}),
        route_policy: &serde_json::Value::Null,
    });
    assert_eq!(status.violations, vec!["cross_zone_fallback_not_allowed".to_owned()]);

    let stream = ModelProviderStreamChunk {
        index: 0,
        content: "delta".to_owned(),
        provider_event: Some("message.delta".to_owned()),
    };
    assert_eq!(stream.content, "delta");

    let media = ModelMediaImageGenerationResp {
        provider_payload: serde_json::json!({"id": "img-1"}),
        asset_url: "https://cdn.example.com/img.png".to_owned(),
        provider_asset_id: Some("img-1".to_owned()),
    };
    assert_eq!(media.provider_asset_id.as_deref(), Some("img-1"));

    assert_eq!(ModelRerankScore { index: 1, score: 0.9 }.index, 1);
    assert_eq!(
        ModelEmbeddingVector {
            index: 2,
            vector: vec![0.1, 0.2]
        }
        .vector
        .len(),
        2
    );
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-model --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Model Source Into Focused Modules

**Files:**
- Create: `crates/novex-model/src/taxonomy.rs`
- Create: `crates/novex-model/src/route.rs`
- Create: `crates/novex-model/src/provider.rs`
- Create: `crates/novex-model/src/usage.rs`
- Create: `crates/novex-model/src/cost.rs`
- Create: `crates/novex-model/src/policy.rs`
- Create: `crates/novex-model/src/util.rs`
- Modify: `crates/novex-model/src/lib.rs`

**Interfaces:**
- Consumes: existing `crates/novex-model/src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move model taxonomy**

Move these items into `src/taxonomy.rs`:

```rust
ModelKind
ModelProviderType
ModelRoutePurpose
ModelRuntimeTarget
```

`taxonomy.rs` should import:

```rust
use crate::util::normalize_registry_token;
use serde::{Deserialize, Serialize};
```

- [ ] **Step 2: Move runtime routes and config**

Move these items into `src/route.rs`:

```rust
ModelRuntimeRoute
impl fmt::Debug for ModelRuntimeRoute
impl ModelRuntimeRoute
ModelRuntimeConfig
impl ModelRuntimeConfig
ModelRuntimeSummary
impl ModelRuntimeSummary
ModelRuntimeRouteSummary
RouteSpec
add_route
read_env
```

`route.rs` should import:

```rust
use crate::taxonomy::{ModelKind, ModelProviderType, ModelRoutePurpose, ModelRuntimeTarget};
use crate::util::{join_url, mask_api_key, normalize_base_url};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{env, fmt};
```

- [ ] **Step 3: Move provider DTOs**

Move these items into `src/provider.rs`:

```rust
ModelProviderStreamChunk
ModelMediaImageGenerationResp
ModelRerankScore
ModelEmbeddingVector
```

`provider.rs` should import:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
```

- [ ] **Step 4: Move usage accounting**

Move these items into `src/usage.rs`:

```rust
ModelTokenUsage
impl ModelTokenUsage
ModelTokenUsageCounts
normalize_model_provider_usage
estimate_model_text_tokens
```

`usage.rs` should import:

```rust
use crate::util::json_i64_field;
use serde::{Deserialize, Serialize};
use serde_json::Value;
```

- [ ] **Step 5: Move cost estimation**

Move these items into `src/cost.rs`:

```rust
ModelUsageCostInput
estimate_model_cost_cents
token_cost_cents
```

`cost.rs` should import:

```rust
use crate::util::{json_f64_field, json_string_field, non_negative};
use serde::{Deserialize, Serialize};
use serde_json::Value;
```

- [ ] **Step 6: Move route policy evaluation**

Move these items into `src/policy.rs`:

```rust
ModelRoutePolicyInput
ModelRoutePolicyStatus
evaluate_model_route_policy
normalize_network_zone
policy_bool_field
policy_u32_field
```

`policy.rs` should import:

```rust
use crate::util::{json_bool_field, json_i64_field};
use serde::{Deserialize, Serialize};
use serde_json::Value;
```

- [ ] **Step 7: Move shared utility helpers**

Move these items into `src/util.rs`:

```rust
json_i64_field
json_f64_field
json_string_field
json_bool_field
json_field
normalize_json_key
json_i64
json_f64
json_bool
normalize_registry_token
non_negative
normalize_base_url
join_url
mask_api_key
```

Keep helper visibility narrow:

```rust
pub(crate) fn json_i64_field(value: &Value, keys: &[&str]) -> Option<i64>;
pub(crate) fn json_f64_field(value: &Value, keys: &[&str]) -> Option<f64>;
pub(crate) fn json_string_field(value: &Value, keys: &[&str]) -> Option<String>;
pub(crate) fn json_bool_field(value: &Value, keys: &[&str]) -> Option<bool>;
pub(crate) fn normalize_registry_token(value: &str) -> String;
pub(crate) fn non_negative(value: i64) -> f64;
pub(crate) fn normalize_base_url(base_url: &str) -> String;
pub(crate) fn join_url(base_url: &str, path: &str) -> String;
pub fn mask_api_key(api_key: &str) -> String;
```

- [ ] **Step 8: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod cost;
mod policy;
mod provider;
mod route;
mod taxonomy;
mod usage;
mod util;

use novex_ai_core::FoundationModule;

pub use cost::{estimate_model_cost_cents, ModelUsageCostInput};
pub use policy::{evaluate_model_route_policy, ModelRoutePolicyInput, ModelRoutePolicyStatus};
pub use provider::{
    ModelEmbeddingVector, ModelMediaImageGenerationResp, ModelProviderStreamChunk,
    ModelRerankScore,
};
pub use route::{
    ModelRuntimeConfig, ModelRuntimeRoute, ModelRuntimeRouteSummary, ModelRuntimeSummary,
};
pub use taxonomy::{ModelKind, ModelProviderType, ModelRoutePurpose, ModelRuntimeTarget};
pub use usage::{
    estimate_model_text_tokens, normalize_model_provider_usage, ModelTokenUsage,
    ModelTokenUsageCounts,
};
pub use util::mask_api_key;

pub const CRATE_ID: &str = "novex-model";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Model Registry",
        "ai-foundation",
        "Model providers, deployments, profiles, routing, usage, and health boundaries.",
    )
}
```

- [ ] **Step 9: Run the structure test and full crate test**

Run:

```bash
cargo test -p novex-model --test module_structure
cargo test -p novex-model
```

Expected: PASS.

---

### Task 3: Move Inline Tests Into Focused Integration Tests

**Files:**
- Create: `crates/novex-model/tests/module_contract.rs`
- Create: `crates/novex-model/tests/route.rs`
- Create: `crates/novex-model/tests/taxonomy.rs`
- Create: `crates/novex-model/tests/usage.rs`
- Create: `crates/novex-model/tests/cost.rs`
- Create: `crates/novex-model/tests/policy.rs`
- Modify: `crates/novex-model/src/lib.rs`

**Interfaces:**
- Consumes: current tests in `#[cfg(test)] mod tests`.
- Produces: focused integration-test files with unchanged assertions.

- [ ] **Step 1: Move test groups**

Use root imports such as `use novex_model::*;` in integration tests, plus `use novex_ai_core::FoundationStatus;` only in `module_contract.rs`.

Move tests according to this map:

```text
module_describes_model_boundary -> tests/module_contract.rs
runtime_config_maps_user_env_to_masked_routes -> tests/route.rs
runtime_config_reports_missing_env_without_creating_partial_routes -> tests/route.rs
dynamic_route_constructor_preserves_registry_route_id -> tests/route.rs
guardian_review_route_purpose_uses_default_llm_route -> tests/route.rs
dynamic_route_parsers_accept_registry_values -> tests/taxonomy.rs
model_usage_normalizes_provider_token_aliases_and_estimates_text_tokens -> tests/usage.rs
model_usage_cost_estimate_applies_token_cost_spec -> tests/cost.rs
model_route_policy_defaults_to_disabled_fallback -> tests/policy.rs
model_route_policy_blocks_cross_zone_fallback_without_explicit_policy -> tests/policy.rs
model_route_policy_allows_cross_zone_fallback_when_policy_explicit -> tests/policy.rs
```

- [ ] **Step 2: Verify `lib.rs` no longer owns tests**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-model/src/lib.rs
```

Expected: no output and exit code 1.

- [ ] **Step 3: Run model tests**

Run:

```bash
cargo test -p novex-model
```

Expected: PASS with `src/lib.rs` reporting 0 unit tests and the moved integration tests passing.

---

### Task 4: Update Model Source-Location Docs

**Files:**
- Modify docs reported by `rg "crates/novex-model/src/lib.rs|novex-model/src/lib.rs" docs/plans docs/superpowers`.
- Modify `docs/ARCHITECTURE.md` if its `crates/novex-model/` layout does not match the implementation.

**Interfaces:**
- Consumes: new Model module paths.
- Produces: docs that point future Model work at focused modules instead of `src/lib.rs`.

- [ ] **Step 1: Find stale Model `lib.rs` instructions**

Run:

```bash
rg -n 'crates/novex-model/src/lib.rs|novex-model/src/lib.rs' docs/plans docs/superpowers
```

Expected: matches in older plans.

- [ ] **Step 2: Update contributor-facing references**

Replace future-work instructions according to ownership:

```text
Model taxonomy -> crates/novex-model/src/taxonomy.rs
Runtime route/config -> crates/novex-model/src/route.rs
Provider-neutral DTOs -> crates/novex-model/src/provider.rs
Usage normalization/token estimation -> crates/novex-model/src/usage.rs
Cost estimation -> crates/novex-model/src/cost.rs
Route fallback policy -> crates/novex-model/src/policy.rs
JSON/env URL/key helpers -> crates/novex-model/src/util.rs
crate facade only -> crates/novex-model/src/lib.rs
```

Do not rewrite historical skeleton creation records.

---

### Task 5: Final Verification and Commit

**Files:**
- Commit all `novex-model` source, test, backend source-contract, and doc changes.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-model` module architecture slice.

- [ ] **Step 1: Run formatting check**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 2: Run focused and downstream model verification**

Run:

```bash
cargo test -p novex-model
cargo test -p novex-rag
cargo test -p novex-provider-client
cargo test -p novex-tools
cargo test -p backend-rust application::ai::foundation_service::tests::summary_lists_required_foundation_crates
```

Expected: PASS.

- [ ] **Step 3: Run diff check**

Run:

```bash
git diff --check
```

Expected: PASS.

- [ ] **Step 4: Commit the slice**

Run:

```bash
git add crates/novex-model/src crates/novex-model/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex model into focused modules"
```

Expected: commit succeeds.

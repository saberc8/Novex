# AI Foundation Crates Module Architecture Design

## Brief

Bring the Novex Rust AI Foundation crates from large single-file crates to a maintainable module architecture that matches `docs/ARCHITECTURE.md`.

The goal is structural normalization, not new product behavior. The refactor should make each crate readable by domain boundary, keep public APIs stable through crate facades, move tests to focused module or integration-test locations, and give future AI foundation work a clear place to land.

## Current State

- `crates/novex-provider-client` already follows the intended facade pattern: `src/lib.rs` declares private modules and re-exports the public API.
- Most other AI Foundation crates still keep implementation and tests in `src/lib.rs`.
- The largest single files are:
  - `novex-rag/src/lib.rs`: parsing, chunking, Milvus request shaping, keyword/BM25 retrieval, answer building, and tests.
  - `novex-mcp/src/lib.rs`: MCP core types, JSON-RPC, Streamable HTTP planning/parsing, OAuth planning/session logic, stdio launch planning, registration validation, and tests.
  - `novex-eval/src/lib.rs`: eval DTOs, trace extraction, metric scoring, regression reporting, trace summary helpers, and tests.
  - `novex-tools/src/lib.rs`: registry types, execution policy, concurrency/batch planning, executor binding/dispatch planning, tool definitions, input adapters, media parsing, and tests.
  - `novex-model/src/lib.rs`: model taxonomy, runtime routes/config, provider DTOs, usage/cost accounting, route policy, env loading, URL/key helpers, and tests.
- `docs/ARCHITECTURE.md` already defines the intended crate responsibilities and submodule names. The implementation should converge on that document instead of inventing a competing layout.
- Some backend tests inspect crate source files with `include_str!` or file reads. Those tests must be updated when behavior moves out of `lib.rs`.

## Design Choice

Use an incremental deep refactor by crate, beginning with the highest-risk and largest crates.

This is preferred over cutting every crate at once because it preserves a compilable workspace after each batch and lets module boundaries be corrected from compiler/test feedback. It is also preferred over a documentation-only pass because the current problem is already visible in the implementation.

The work still has a strict end state: every AI Foundation crate should have a small `lib.rs` facade, focused modules, and focused tests unless the crate is genuinely tiny.

## Normative Module Rules

Each crate should follow these rules:

1. `src/lib.rs` is a facade. It may contain `mod`, `pub mod`, `pub use`, `CRATE_ID`, and the crate-level `module()` constructor, but not large business logic.
2. Public APIs remain available from the crate root when existing callers already depend on them. Internal placement can change, but consumers should not need broad import rewrites.
3. Modules are named after domain responsibilities, not implementation accidents.
4. Private helper functions stay near the public behavior they support.
5. Tests that exercise a private helper live in that helper's module. Tests that exercise crate-level behavior or public contracts live in `tests/`.
6. Cross-crate dependency direction must continue to match `docs/ARCHITECTURE.md`.
7. New modules must not introduce cycles by moving shared types into downstream crates. Shared AI foundation vocabulary belongs in `novex-ai-core` or the owning domain crate.
8. The refactor must not add new runtime behavior, database schema changes, UI behavior, or external provider behavior.

## Target Crate Layouts

### novex-rag

Target modules:

- `knowledge`: dataset/document/chunk/citation DTOs.
- `model_routes`: RAG model route selection helpers.
- `parse`: plain text, structured document parsing, source blocks, markers, table/text block parsing.
- `chunk`: chunk strategy, metadata, semantic search text construction.
- `milvus`: Milvus request/response DTOs, search-hit parsing, collection/upsert request builders.
- `retrieval`: keyword retrieval, BM25, tokenization, query expansion.
- `answer`: extractive answer construction and citation shaping.
- `module`: `FoundationModule` constructor if it grows beyond a tiny function.

`lib.rs` should re-export the current public DTOs and functions so callers can keep using `novex_rag::*`.

### novex-mcp

Target modules:

- `types`: server status, transport kind, auth scope/type, tool descriptors, invocation DTOs.
- `tool_code`: MCP tool code normalization.
- `json_rpc`: JSON-RPC request and notification builders.
- `streamable_http`: request plans, response parsing, SSE/JSON payload handling.
- `oauth`: authorization URL planning, token exchange/refresh planning, token response/session material, OAuth validation errors.
- `stdio`: env values, lifecycle policy, launch plans, tool-call plans, stdio validation errors.
- `client_error`: MCP client error kinds and helpers.
- `registration`: registration policy, discovery plan, endpoint allow-list validation.
- `module`: `FoundationModule` constructor if needed.

OAuth and stdio should not stay intertwined with HTTP response parsing once split.

### novex-eval

Target modules:

- `case`: eval target/metric enums, case input/expected/actual/candidate DTOs, trace-to-actual conversion.
- `score`: metric dispatch and individual scoring functions.
- `trace_extract`: trace bundle event extraction, compaction/guardian/supervisor/tool/inference summaries.
- `report`: regression report aggregation.
- `text`: normalization, case-insensitive matching, snippets, rounding helpers.
- `module`: `FoundationModule` constructor if needed.

Trace parsing helpers should be private to `trace_extract` unless they are part of a deliberate public contract.

### novex-tools

Target modules:

- `types`: tool kind, risk, approval policy, definitions, model tool specs, execution records.
- `policy`: tool execution policy evaluation and risk/policy code helpers.
- `concurrency`: locks, concurrency policy, batch mode, batch planning.
- `executor`: executor kinds, bindings, registry, dispatch plans, registry errors.
- `router`: routed tool calls, route errors, tool router.
- `definitions`: built-in agent model-loop and customer-service tool definitions.
- `adapters`: Feishu, GitHub, media image input parsing.
- `media`: image request/result DTOs and provider response parsing.
- `module`: `FoundationModule` constructor if needed.

Connector-specific parsing can remain in `novex-tools` only where it adapts tool input to connector DTOs. Connector transport and provider semantics stay in `novex-connectors`.

### novex-model

Target modules:

- `taxonomy`: model kind, provider type, route purpose, runtime target.
- `route`: runtime route/config/summary DTOs and route lookup helpers.
- `provider`: provider stream chunk, media generation response, rerank score, embedding vector.
- `usage`: usage counts, usage normalization, token estimation.
- `cost`: cost input and cost estimation helpers.
- `policy`: route policy input/status and policy evaluation.
- `env`: runtime config loading from environment.
- `util`: URL joining, key masking, JSON field helpers where not domain-specific.
- `module`: `FoundationModule` constructor if needed.

Provider transport DTOs remain here only when they are provider-neutral shared contracts. HTTP dispatch stays in `novex-provider-client` or backend transport adapters.

### Smaller Crates

After the largest crates are normalized, apply the same rules to medium crates:

- `novex-agent-runtime`: split protocol/runtime state, turn parsing, budget/compaction, supervisor helpers.
- `novex-approval-review`: split review vocabulary, prompt/parse helpers, decision mapping.
- `novex-ai-core`: split tenant/resource/run graph/policy/module metadata if the file remains hard to scan.
- `novex-connectors`: split registry, GitHub, Feishu/web/database DTOs as connector coverage grows.
- `novex-agent`, `novex-trigger`, `novex-trace`, `novex-skill`, `novex-memory`, and `novex-agent-protocol` can stay compact until they exceed the facade rule or start mixing unrelated domains.

## Test Layout

Use three levels of tests:

1. Module unit tests in the same module file for private helper behavior.
2. Crate integration tests in `crates/<crate>/tests/*.rs` for public facade behavior and cross-module contracts.
3. Backend source-boundary tests updated to assert the new module ownership rather than searching only `src/lib.rs`.

Test movement should be behavior-preserving. Rename tests only when needed to clarify the module contract.

## Migration Order

1. Add this design spec and review it against the current workspace.
2. Refactor `novex-rag` first because it has the largest file and the clearest existing architecture boundaries.
3. Refactor `novex-mcp` next because it mixes several independent protocol concerns.
4. Refactor `novex-tools`, then `novex-model`, because they are heavily imported by backend and other crates.
5. Refactor `novex-eval` after trace/model/tool public roots are stable.
6. Sweep medium and small crates with the same facade rules.
7. Update docs that still instruct contributors to add large feature slices directly to `src/lib.rs`.

Within each crate, migrate one module group at a time:

1. Move types and helpers into the target module.
2. Re-export existing public items from `lib.rs`.
3. Move focused tests next to the module or into `tests/`.
4. Run formatting and focused tests.
5. Update any source-inspection tests or docs that reference the old file location.

## Error Handling

The refactor should preserve all existing error types and messages unless a compiler-required import change exposes a clear naming bug.

If splitting reveals that two modules need the same private error helper, prefer moving the helper to the owning domain module and importing it with `pub(crate)` visibility. Do not create a generic error module unless multiple public behaviors genuinely share the same error vocabulary.

## Public API Compatibility

The default compatibility rule is root-level re-export preservation:

- Existing code using `novex_rag::chunk_document` should keep compiling.
- Existing code using `novex_mcp::McpOAuthAuthorizationPlan` should keep compiling.
- Existing code using `novex_tools::ToolDefinition` should keep compiling.
- Existing code using `novex_model::ModelRuntimeConfig` should keep compiling.

New module paths may also be public when useful, but they are secondary to preserving the crate-root facade.

## Documentation Updates

Update `docs/ARCHITECTURE.md` only if the implementation proves the documented boundaries need adjustment. Otherwise, the code should conform to the existing architecture document.

Update feature plans or contributor-facing docs when they instruct future work to add substantial logic directly into a crate `src/lib.rs`.

## Acceptance Criteria

1. The largest AI Foundation crates no longer keep mixed domain implementation in `src/lib.rs`.
2. Every normalized crate has a small crate facade with explicit module declarations and root-level re-exports.
3. Tests are no longer centralized in large `mod tests` blocks inside huge `lib.rs` files for normalized crates.
4. Public crate-root imports used by backend and peer crates remain compatible.
5. Dependency direction still matches `docs/ARCHITECTURE.md`.
6. Backend source-inspection tests and docs point at the new module ownership when relevant.
7. Formatting and tests pass for each migrated crate before moving to the next batch.
8. Final workspace verification passes or any pre-existing unrelated failures are documented with evidence.

## Verification Plan

Run these gates during the migration:

- `cargo fmt --all -- --check`
- `cargo test -p novex-rag`
- `cargo test -p novex-mcp`
- `cargo test -p novex-tools`
- `cargo test -p novex-model`
- `cargo test -p novex-eval`
- `cargo test --workspace`
- `git diff --check`

Use `cargo test --workspace` as the final behavioral gate. If it fails because of environment-only integration tests or pre-existing failures, capture the exact failing command and the failing tests before deciding whether the architecture work is complete.

## Non-Goals

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new AI feature behavior.
- No crate renaming or workspace member reshaping unless a dependency cycle makes it unavoidable.
- No removal of public root exports without a deliberate compatibility decision.

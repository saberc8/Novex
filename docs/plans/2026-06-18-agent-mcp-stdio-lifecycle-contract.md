# Agent MCP Stdio Lifecycle Contract Plan

## Goal

Move the MCP gateway one step closer to Codex-style local MCP server support by adding a transport-neutral stdio launch and lifecycle contract in `novex-mcp`.

## Architecture

Keep `novex-mcp` as the pure protocol/policy crate. It should define how a stdio MCP server is represented, validated, bounded, and serialized into safe operational evidence, but it should not spawn subprocesses, resolve secrets, persist sessions, or own backend audit. Future backend/process-supervisor slices can consume this contract to run local MCP servers without inventing a second lifecycle vocabulary.

This is an adapter-port slice from Codex MCP infrastructure: command launch metadata, environment secret references, startup/shutdown timeout policy, initialize/list-tools/call-tools/shutdown phases, and sanitized evidence for trace/eval/audit.

## Scope

- Add `McpStdioEnvValue`, `McpStdioLaunchConfig`, `McpStdioLifecyclePolicy`, `McpStdioLifecyclePhase`, and `McpStdioLaunchPlan` to `crates/novex-mcp`.
- Validate stdio launch commands and environment bindings before a supervisor can consume them.
- Require env secret references to use the existing `env:` convention.
- Clamp or reject unsafe lifecycle timeout values through an explicit policy constructor.
- Produce sanitized launch evidence that includes command, args, working directory, timeout policy, lifecycle phases, and env binding kinds without leaking literal env values or resolved secrets.
- Update the migration matrix and enterprise foundation plan so MCP shows stdio lifecycle contract progress while process supervision, OAuth, persisted sessions, and external deployed smoke coverage remain future slices.

## Out of Scope

- Spawning stdio MCP subprocesses.
- Backend DB schema or API changes for persisting launch config.
- Secret resolution or environment injection.
- OAuth/browser authorization.
- Persisted MCP sessions.
- External MCP server smoke tests.

## RED Tests

- `mcp_stdio_launch_plan_sanitizes_env_secret_refs`: a launch plan with literal and `env:` secret bindings exposes only binding kind/secretRef evidence and never leaks literal env values.
- `mcp_stdio_launch_plan_rejects_empty_command`: empty or whitespace-only commands fail validation before any supervisor can spawn them.
- `mcp_stdio_launch_plan_rejects_invalid_env_secret_ref`: secret bindings without the `env:` prefix fail closed.
- `mcp_stdio_lifecycle_policy_rejects_out_of_bounds_timeouts`: lifecycle policy rejects startup/shutdown timeouts outside the supported bounds.
- `mcp_stdio_lifecycle_plan_lists_expected_phases`: the plan exposes the deterministic lifecycle phase order future supervisors should follow.

## Implementation Steps

1. Add RED tests in `crates/novex-mcp/tests/stdio.rs` for stdio launch sanitization, empty command rejection, invalid secret refs, timeout bounds, and lifecycle phases.
2. Run `cargo test -p novex-mcp mcp_stdio --offline` and confirm the tests fail because the stdio contract does not exist.
3. Add stdio env value, launch config, lifecycle policy, lifecycle phase, launch plan, and error types in `crates/novex-mcp/src/stdio.rs`.
4. Implement validation helpers for command, env names, env secret refs, working directory trimming, and timeout bounds.
5. Implement `McpStdioLaunchPlan::new` and `sanitized_evidence`.
6. Run focused `novex-mcp` tests, then backend MCP tests to confirm existing HTTP execution remains untouched.
7. Update `docs/plans/2026-06-16-codex-migration-matrix.md` and `docs/plans/2026-06-16-enterprise-agent-foundation.md`.
8. Run full verification, commit implementation, fast-forward merge to `main`, run `cargo clean`, and remove this worktree/branch.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p novex-mcp mcp_stdio --offline`
- `cargo test -p novex-mcp --offline`
- `cargo test -p backend-rust mcp --offline`
- `cargo test --workspace --offline`

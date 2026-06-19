# Migration Source Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a human-oriented migration source tree while preserving `backend/migrations` as the SQLx execution directory.

**Architecture:** `backend/migration_sources` becomes the organized source of truth grouped by module and migration kind. `backend/migrations` remains flat so `sqlx::migrate!("./migrations")`, `sqlx migrate run`, and existing `include_str!` tests continue to work. A script and Rust integration test verify the source tree and flat directory stay byte-for-byte aligned.

**Tech Stack:** Rust integration tests, POSIX shell, SQLx file migrations, PostgreSQL SQL files.

## Global Constraints

- Do not rewrite existing SQL content.
- Do not move or rename files in `backend/migrations`.
- Preserve current SQLx migration behavior.
- Keep the source tree grouped by module and by `schema`, `seed`, or `patch`.

---

### Task 1: Migration Source Mirror Test

**Files:**
- Create: `backend/tests/migration_sources.rs`

**Interfaces:**
- Consumes: `backend/migrations/*.sql`
- Produces: Integration test `migration_sources_mirror_sqlx_flat_directory`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn migration_sources_mirror_sqlx_flat_directory() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let flat_root = manifest_dir.join("migrations");
    let source_root = manifest_dir.join("migration_sources");

    assert!(source_root.is_dir(), "backend/migration_sources must exist");
    // The full test recursively collects SQL files, verifies unique basenames,
    // checks schema/seed/patch parent directories, and compares file content.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend migration_sources_mirror_sqlx_flat_directory --offline`

Expected: FAIL because `backend/migration_sources` does not exist yet.

- [ ] **Step 3: Keep the failing test in place**

No implementation code is added in this task.

### Task 2: Migration Source Tree

**Files:**
- Create: `backend/migration_sources/**/**/*.sql`
- Create: `backend/migration_sources/README.md`

**Interfaces:**
- Consumes: Existing flat migration filenames.
- Produces: Organized source files with the same basename and content as the flat SQLx migrations.

- [ ] **Step 1: Create module/kind directories**

Use module names such as `system`, `scheduler`, `ai/model`, `ai/agent`, `ai/mcp`, and `ai/template`. Use `schema` for `create_*` files, `seed` for `seed_*` files, and `patch` for `add_*`, `patch_*`, `remove_*`, `sanitize_*`, `enrich_*`, or `promote_*` files.

- [ ] **Step 2: Copy current SQL files into the source tree**

Each source file keeps the exact same basename as its flat counterpart.

- [ ] **Step 3: Run the mirror test**

Run: `cargo test -p backend migration_sources_mirror_sqlx_flat_directory --offline`

Expected: PASS.

### Task 3: Sync and Check Script

**Files:**
- Create: `scripts/sync-migration-sources.sh`
- Modify: `backend/README.md`

**Interfaces:**
- Consumes: `backend/migration_sources/**/*.sql`
- Produces: `backend/migrations/*.sql` via sync mode and drift detection via `--check`.

- [ ] **Step 1: Add script**

```bash
scripts/sync-migration-sources.sh --check
scripts/sync-migration-sources.sh
```

`--check` fails on missing files, extra files, duplicate basenames, or content drift. Default mode copies source SQL files into `backend/migrations`.

- [ ] **Step 2: Document workflow**

Add backend README guidance that new migrations should be authored under `backend/migration_sources` and synchronized into `backend/migrations`.

### Task 4: Verification

**Files:**
- Test only.

**Interfaces:**
- Consumes: All changes from tasks 1-3.
- Produces: Passing focused checks.

- [ ] **Step 1: Run mirror test**

Run: `cargo test -p backend migration_sources_mirror_sqlx_flat_directory --offline`

Expected: PASS.

- [ ] **Step 2: Run sync script check**

Run: `scripts/sync-migration-sources.sh --check`

Expected: PASS with no output except the success summary.

- [ ] **Step 3: Run existing migration filename test**

Run: `cargo test -p backend migration_filenames_use_unique_versions --offline`

Expected: PASS.

# Novex Skill Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-skill` from a single `src/lib.rs` into focused skill package path and resource classification modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as a facade. Move package path normalization, manifest selection, root stripping, and package errors into `path.rs`; move resource kind classification and text-resource detection into `resource.rs`. Move inline tests into crate integration tests grouped by module ownership.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No skill package behavior changes.
- No frontend changes.
- Preserve root-level exports such as `SkillResourceKind`, `SkillPackageError`, `SkillPackageFile`, `SkillPackagePath`, `normalize_skill_package_path`, `selected_skill_md_index`, and `skill_resource_kind`.
- Keep `novex-skill` dependency-free from backend crates.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-skill`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-skill/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-skill/src/path.rs`
  - Owns `SkillPackageError`, `SkillPackagePath`, `SkillPackageFile`, path normalization, manifest selection, skill root derivation, and root stripping.
- Create: `crates/novex-skill/src/resource.rs`
  - Owns `SkillResourceKind` and `skill_resource_kind`.
- Modify: `crates/novex-skill/src/lib.rs`
  - Keep only module declarations and root re-exports.

---

### Task 1: Add Skill Structure Tests

**Files:**
- Create: `crates/novex-skill/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_skill`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-skill/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_skill::{
    normalize_skill_package_path, selected_skill_md_index, skill_resource_kind,
    skill_root_from_skill_md_path, strip_skill_root, SkillPackageFile, SkillPackagePath,
    SkillResourceKind,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_skill_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["path", "resource"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum SkillResourceKind",
        "pub enum SkillPackageError",
        "pub trait SkillPackagePath",
        "pub struct SkillPackageFile",
        "pub fn normalize_skill_package_path",
        "pub fn selected_skill_md_index",
        "pub fn skill_resource_kind",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn skill_domain_modules_exist() {
    for module in ["src/path.rs", "src/resource.rs"] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_skill_contracts() {
    assert_eq!(
        normalize_skill_package_path(r".\\writer\\references\\guide.md").unwrap(),
        "writer/references/guide.md"
    );
    let files = [
        SkillPackageFile {
            relative_path: "writer/references/style.md",
        },
        SkillPackageFile {
            relative_path: "writer/SKILL.md",
        },
    ];
    let index = selected_skill_md_index(&files).unwrap();
    assert_eq!(files[index].relative_path(), "writer/SKILL.md");
    assert_eq!(skill_root_from_skill_md_path("writer/SKILL.md"), "writer");
    assert_eq!(
        strip_skill_root("writer", "writer/references/style.md"),
        Some("references/style.md".to_owned())
    );
    assert_eq!(skill_resource_kind("SKILL.md"), SkillResourceKind::SkillMd);
    assert!(SkillResourceKind::Script.is_text_resource("application/json"));
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-skill --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Source and Tests

**Files:**
- Create: `crates/novex-skill/src/path.rs`
- Create: `crates/novex-skill/src/resource.rs`
- Create: `crates/novex-skill/tests/path.rs`
- Create: `crates/novex-skill/tests/resource.rs`
- Modify: `crates/novex-skill/src/lib.rs`

**Interfaces:**
- Consumes: existing `src/lib.rs` implementations.
- Produces: same public API through crate-root re-exports.

- [ ] **Step 1: Move modules**

Move items according to this map:

```text
SkillPackageError, SkillPackagePath, SkillPackageFile,
normalize_skill_package_path, normalize_skill_package_path_or_empty,
selected_skill_md_index, skill_root_from_skill_md_path, strip_skill_root -> src/path.rs
SkillResourceKind, skill_resource_kind -> src/resource.rs
```

- [ ] **Step 2: Replace `src/lib.rs` with the facade**

Use this facade:

```rust
mod path;
mod resource;

pub use path::{
    normalize_skill_package_path, normalize_skill_package_path_or_empty,
    selected_skill_md_index, skill_root_from_skill_md_path, strip_skill_root, SkillPackageError,
    SkillPackageFile, SkillPackagePath,
};
pub use resource::{skill_resource_kind, SkillResourceKind};
```

- [ ] **Step 3: Move tests**

Use root imports in integration tests. Move package path/manifest tests to `tests/path.rs` and resource classification tests to `tests/resource.rs`.

- [ ] **Step 4: Verify**

Run:

```bash
rg -n '#\[cfg\(test\)\]|mod tests' crates/novex-skill/src/lib.rs
cargo test -p novex-skill
```

Expected: `rg` has no output with exit code 1, and tests pass.

---

### Task 3: Final Verification and Commit

**Files:**
- Commit source, tests, and doc updates.

**Interfaces:**
- Consumes: completed module split.
- Produces: committed, verified `novex-skill` module architecture slice.

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p novex-skill
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit the slice**

Run:

```bash
git add crates/novex-skill/src crates/novex-skill/tests docs/ARCHITECTURE.md docs/superpowers/specs/2026-06-19-ai-foundation-crates-module-architecture-design.md
git diff --cached --check
git commit -m "refactor: split novex skill into focused modules"
```

Expected: commit succeeds.

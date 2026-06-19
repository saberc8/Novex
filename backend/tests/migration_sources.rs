use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

#[test]
fn migration_sources_mirror_sqlx_flat_directory() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let flat_root = manifest_dir.join("migrations");
    let source_root = manifest_dir.join("migration_sources");

    assert!(
        source_root.is_dir(),
        "backend/migration_sources must exist and mirror backend/migrations"
    );

    let flat_files = collect_sql_files(&flat_root);
    let source_files = collect_sql_files(&source_root);

    assert!(
        !flat_files.is_empty(),
        "backend/migrations must contain SQLx migrations"
    );
    assert_eq!(
        source_files.len(),
        flat_files.len(),
        "migration_sources and migrations must contain the same number of SQL files"
    );

    for (name, source_path) in &source_files {
        assert_source_layout(&source_root, source_path, name);
        let flat_path = flat_files
            .get(name)
            .unwrap_or_else(|| panic!("source migration {name} has no flat SQLx counterpart"));

        let source_bytes = fs::read(source_path)
            .unwrap_or_else(|error| panic!("read source migration {source_path:?}: {error}"));
        let flat_bytes = fs::read(flat_path)
            .unwrap_or_else(|error| panic!("read flat migration {flat_path:?}: {error}"));
        assert_eq!(
            source_bytes, flat_bytes,
            "source migration {source_path:?} must match flat migration {flat_path:?}"
        );
    }

    for name in flat_files.keys() {
        assert!(
            source_files.contains_key(name),
            "flat SQLx migration {name} is missing from backend/migration_sources"
        );
    }
}

#[test]
fn retired_template_delivery_cleanup_drops_only_unused_tables() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let migration_path =
        manifest_dir.join("migrations/202606200003_drop_retired_template_delivery_tables.sql");
    let migration = fs::read_to_string(&migration_path)
        .unwrap_or_else(|error| panic!("read cleanup migration {migration_path:?}: {error}"));

    for table in [
        "ai_template_smoke_result",
        "ai_template_smoke_run",
        "ai_customer_frontend_config",
        "ai_customer_package",
    ] {
        assert!(
            migration.contains(&format!("DROP TABLE IF EXISTS {table}")),
            "cleanup migration must drop retired table {table}"
        );
    }

    for active_contract in [
        "ai:customer-service:agent:run",
        "ai:customer-service:read",
        "ai:customer-service:ticket",
        "ai:customer-service:handoff",
    ] {
        assert!(
            !migration.contains(active_contract),
            "cleanup migration must not remove active customer-service contract {active_contract}"
        );
    }
}

fn collect_sql_files(root: &Path) -> BTreeMap<String, PathBuf> {
    let mut files = BTreeMap::new();
    visit_sql_files(root, &mut files);
    files
}

fn visit_sql_files(root: &Path, files: &mut BTreeMap<String, PathBuf>) {
    for entry in
        fs::read_dir(root).unwrap_or_else(|error| panic!("read directory {root:?}: {error}"))
    {
        let entry =
            entry.unwrap_or_else(|error| panic!("read directory entry in {root:?}: {error}"));
        let path = entry.path();
        if path.is_dir() {
            visit_sql_files(&path, files);
            continue;
        }

        if path.extension() != Some(OsStr::new("sql")) {
            continue;
        }

        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_else(|| panic!("migration path must be UTF-8: {path:?}"))
            .to_owned();

        if let Some(existing) = files.insert(name.clone(), path.clone()) {
            panic!("duplicate migration basename {name}: {existing:?} and {path:?}");
        }
    }
}

fn assert_source_layout(source_root: &Path, source_path: &Path, name: &str) {
    let relative = source_path
        .strip_prefix(source_root)
        .unwrap_or_else(|error| panic!("source path {source_path:?} must be under root: {error}"));
    let parts: Vec<_> = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();

    assert!(
        parts.len() >= 3,
        "source migration {relative:?} must live under <module>/<kind>/{name}"
    );

    let kind_dir = parts[parts.len() - 2];
    let expected_kind = expected_kind_from_filename(name);
    assert_eq!(
        kind_dir, expected_kind,
        "source migration {relative:?} must live in a {expected_kind} directory"
    );
}

fn expected_kind_from_filename(name: &str) -> &'static str {
    let (_, description) = name
        .split_once('_')
        .unwrap_or_else(|| panic!("migration filename must start with version_: {name}"));

    if description.starts_with("create_") {
        "schema"
    } else if description.starts_with("seed_") {
        "seed"
    } else {
        "patch"
    }
}

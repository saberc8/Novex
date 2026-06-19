use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_memory::{
    build_memory_context, module, MemoryAccessContext, MemoryScope, MemoryScopeRef, MemorySnippet,
    MemoryWritePolicy,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_memory_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["context", "module", "types"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum MemoryScope",
        "pub struct MemoryContext",
        "pub fn build_memory_context",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn memory_domain_modules_exist() {
    for module in ["src/context.rs", "src/module.rs", "src/types.rs"] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_memory_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-memory");
    assert_eq!(module.status, FoundationStatus::Skeleton);

    let context = build_memory_context(
        vec![MemorySnippet {
            tenant_id: "tenant-a".to_owned(),
            scope: MemoryScope::User,
            scope_id: "user-1".to_owned(),
            key: "profile.locale".to_owned(),
            content: "Prefers Chinese answers".to_owned(),
            write_policy: MemoryWritePolicy::UserApproved,
        }],
        &MemoryAccessContext {
            tenant_id: "tenant-a".to_owned(),
            subject_id: "user-1".to_owned(),
            allowed_scopes: vec![MemoryScopeRef {
                scope: MemoryScope::User,
                scope_id: "user-1".to_owned(),
            }],
            max_snippets: 4,
        },
    );

    assert_eq!(context.snippets.len(), 1);
    assert_eq!(context.snippets[0].key, "profile.locale");
}

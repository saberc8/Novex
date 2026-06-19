use novex_memory::{
    build_memory_context, MemoryAccessContext, MemoryScope, MemoryScopeRef, MemorySnippet,
    MemoryWritePolicy,
};

#[test]
fn memory_context_filters_candidates_by_tenant_and_allowed_scope_refs() {
    let context = build_memory_context(
        vec![
            MemorySnippet {
                tenant_id: "tenant-a".to_owned(),
                scope: MemoryScope::User,
                scope_id: "user-1".to_owned(),
                key: "profile.locale".to_owned(),
                content: "Prefers Chinese answers".to_owned(),
                write_policy: MemoryWritePolicy::UserApproved,
            },
            MemorySnippet {
                tenant_id: "tenant-a".to_owned(),
                scope: MemoryScope::Project,
                scope_id: "project-denied".to_owned(),
                key: "project.secret".to_owned(),
                content: "Do not leak".to_owned(),
                write_policy: MemoryWritePolicy::Automatic,
            },
            MemorySnippet {
                tenant_id: "tenant-b".to_owned(),
                scope: MemoryScope::User,
                scope_id: "user-1".to_owned(),
                key: "profile.locale".to_owned(),
                content: "Wrong tenant".to_owned(),
                write_policy: MemoryWritePolicy::UserApproved,
            },
        ],
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
    assert_eq!(context.snippets[0].content, "Prefers Chinese answers");
}

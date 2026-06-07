use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-memory";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Session,
    User,
    Org,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWritePolicy {
    Disabled,
    UserApproved,
    Automatic,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryScopeRef {
    pub scope: MemoryScope,
    pub scope_id: String,
}

impl MemoryScopeRef {
    pub fn matches(&self, scope: MemoryScope, scope_id: &str) -> bool {
        self.scope == scope && self.scope_id == scope_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnippet {
    pub tenant_id: String,
    pub scope: MemoryScope,
    pub scope_id: String,
    pub key: String,
    pub content: String,
    pub write_policy: MemoryWritePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAccessContext {
    pub tenant_id: String,
    pub subject_id: String,
    pub allowed_scopes: Vec<MemoryScopeRef>,
    pub max_snippets: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryContext {
    pub snippets: Vec<MemorySnippet>,
}

impl MemoryContext {
    pub fn empty() -> Self {
        Self::default()
    }
}

pub fn build_memory_context(
    candidates: impl IntoIterator<Item = MemorySnippet>,
    access: &MemoryAccessContext,
) -> MemoryContext {
    let snippets = candidates
        .into_iter()
        .filter(|snippet| snippet.tenant_id == access.tenant_id)
        .filter(|snippet| {
            access
                .allowed_scopes
                .iter()
                .any(|scope_ref| scope_ref.matches(snippet.scope, &snippet.scope_id))
        })
        .take(access.max_snippets)
        .collect();

    MemoryContext { snippets }
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Memory",
        "ai-foundation",
        "Session, user, organization, project memory policy, retention, and retrieval boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_memory_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-memory");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

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
}

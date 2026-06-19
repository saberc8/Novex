use serde::{Deserialize, Serialize};

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

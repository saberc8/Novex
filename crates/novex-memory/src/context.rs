use crate::types::{MemoryAccessContext, MemoryContext, MemorySnippet};

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

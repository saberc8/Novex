mod context;
mod module;
mod types;

pub use context::build_memory_context;
pub use module::module;
pub use types::{
    MemoryAccessContext, MemoryContext, MemoryScope, MemoryScopeRef, MemorySnippet,
    MemoryWritePolicy,
};

pub const CRATE_ID: &str = "novex-memory";

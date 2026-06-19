mod bundle;
mod event;
mod module;
mod summary;

pub use bundle::TraceBundle;
pub use event::{TraceEvent, TraceEventKind};
pub use module::module;
pub use summary::TraceReplaySummary;

pub const CRATE_ID: &str = "novex-trace";

mod item;
mod outcome;

pub use item::{AgentTurnItem, AgentTurnItemType, ToolObservationStatus};
pub use outcome::TurnOutcome;

pub const CRATE_ID: &str = "novex-agent-protocol";

mod parser;
mod state;

pub use parser::{
    parse_model_turn_output, ModelTurnParseError, ParsedModelTurnOutput,
    StreamingModelTurnParseStatus, StreamingModelTurnParser, MAX_STREAMING_MODEL_TURN_BUFFER_CHARS,
};
pub use state::{
    AgentCompactionPhase, AgentCompactionReason, AgentCompactionTrigger, AgentContextCompaction,
    AgentRemoteCompactionImplementation, AgentRemoteCompactionRequest, AgentRuntimeBudget,
    AgentRuntimeState,
};

pub const CRATE_ID: &str = "novex-agent-runtime";

use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedModelTurnOutput {
    pub item: AgentTurnItem,
    #[serde(default)]
    pub items: Vec<AgentTurnItem>,
    pub outcome: TurnOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTurnParseError {
    pub message: String,
}

pub const MAX_STREAMING_MODEL_TURN_BUFFER_CHARS: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamingModelTurnParseStatus {
    Pending,
    Ready(ParsedModelTurnOutput),
}

#[derive(Debug, Clone)]
pub struct StreamingModelTurnParser {
    buffer: String,
    ready: bool,
    max_chars: usize,
}

impl StreamingModelTurnParser {
    pub fn new() -> Self {
        Self::with_max_chars(MAX_STREAMING_MODEL_TURN_BUFFER_CHARS)
    }

    pub fn with_max_chars(max_chars: usize) -> Self {
        Self {
            buffer: String::new(),
            ready: false,
            max_chars,
        }
    }

    pub fn push_delta(
        &mut self,
        delta: &str,
    ) -> Result<StreamingModelTurnParseStatus, ModelTurnParseError> {
        if self.ready {
            return Err(ModelTurnParseError {
                message: "streaming model turn was already parsed".to_owned(),
            });
        }
        if self.buffer.chars().count() + delta.chars().count() > self.max_chars {
            return Err(ModelTurnParseError {
                message: "streaming model turn exceeded buffer limit".to_owned(),
            });
        }

        self.buffer.push_str(delta);
        let trimmed = self.buffer.trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            return Ok(StreamingModelTurnParseStatus::Pending);
        }

        let value = match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => value,
            Err(err) if err.is_eof() => return Ok(StreamingModelTurnParseStatus::Pending),
            Err(err) => {
                return Err(ModelTurnParseError {
                    message: format!("streaming model turn JSON is invalid: {err}"),
                })
            }
        };
        match value.get("type").and_then(Value::as_str) {
            Some("tool_call" | "tool_calls") => {
                let parsed = parse_model_turn_output(trimmed)?;
                self.ready = true;
                Ok(StreamingModelTurnParseStatus::Ready(parsed))
            }
            _ => Err(ModelTurnParseError {
                message: "streaming model turn JSON is not a tool call".to_owned(),
            }),
        }
    }
}

impl Default for StreamingModelTurnParser {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_model_turn_output(output: &str) -> Result<ParsedModelTurnOutput, ModelTurnParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(ModelTurnParseError {
            message: "model output is empty".to_owned(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        match value.get("type").and_then(Value::as_str) {
            Some("tool_call") => {
                let item = parse_tool_call_value(&value, 0)?;
                return Ok(ParsedModelTurnOutput {
                    item: item.clone(),
                    items: vec![item],
                    outcome: TurnOutcome::NeedsFollowUp,
                });
            }
            Some("tool_calls") => {
                let calls = value
                    .get("calls")
                    .and_then(Value::as_array)
                    .ok_or_else(|| ModelTurnParseError {
                        message: "tool_calls.calls is required".to_owned(),
                    })?;
                if calls.is_empty() {
                    return Err(ModelTurnParseError {
                        message: "tool_calls requires at least one call".to_owned(),
                    });
                }

                let items = calls
                    .iter()
                    .enumerate()
                    .map(|(index, call)| parse_tool_call_value(call, index))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(ParsedModelTurnOutput {
                    item: items[0].clone(),
                    items,
                    outcome: TurnOutcome::NeedsFollowUp,
                });
            }
            _ => {}
        }
    }

    let item = AgentTurnItem::FinalAnswer {
        content: trimmed.to_owned(),
    };
    Ok(ParsedModelTurnOutput {
        item: item.clone(),
        items: vec![item],
        outcome: TurnOutcome::Final,
    })
}

fn parse_tool_call_value(
    value: &Value,
    index: usize,
) -> Result<AgentTurnItem, ModelTurnParseError> {
    let call_id = value
        .get("callId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("call-{}", index + 1));
    let tool_code = value
        .get("toolCode")
        .and_then(Value::as_str)
        .ok_or_else(|| ModelTurnParseError {
            message: "toolCode is required".to_owned(),
        })?
        .to_owned();
    let arguments = value.get("arguments").cloned().unwrap_or(Value::Null);

    Ok(AgentTurnItem::tool_call(call_id, tool_code, arguments))
}

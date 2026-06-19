use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuTextMessage {
    pub text: String,
}

impl FeishuTextMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into().trim().to_owned(),
        }
    }

    pub fn to_webhook_payload(&self) -> Value {
        json!({
            "msg_type": "text",
            "content": {
                "text": self.text,
            },
        })
    }
}

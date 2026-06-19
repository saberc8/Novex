use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpClientErrorKind {
    HttpStatus,
    UnsupportedContentType,
    JsonRpcError,
    MalformedJson,
    MissingResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientError {
    pub kind: McpClientErrorKind,
    pub message: String,
    pub http_status: Option<u16>,
    pub rpc_code: Option<i64>,
}

impl McpClientError {
    pub(crate) fn new(kind: McpClientErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            http_status: None,
            rpc_code: None,
        }
    }

    pub(crate) fn with_http_status(mut self, http_status: u16) -> Self {
        self.http_status = Some(http_status);
        self
    }

    pub(crate) fn with_rpc_code(mut self, rpc_code: i64) -> Self {
        self.rpc_code = Some(rpc_code);
        self
    }
}

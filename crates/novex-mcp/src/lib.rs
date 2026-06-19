mod client_error;
mod json_rpc;
mod oauth;
mod registration;
mod stdio;
mod streamable_http;
mod tool_code;
mod types;

use novex_ai_core::FoundationModule;

pub use client_error::{McpClientError, McpClientErrorKind};
pub use json_rpc::{McpJsonRpcNotification, McpJsonRpcRequest};
pub use oauth::{
    mcp_oauth_session_from_token_response, McpOAuthAuthorizationConfig, McpOAuthAuthorizationError,
    McpOAuthAuthorizationPlan, McpOAuthClientAuth, McpOAuthGrantType, McpOAuthPkceMethod,
    McpOAuthSessionError, McpOAuthSessionMaterial, McpOAuthTokenExchangeConfig,
    McpOAuthTokenExchangePlan, McpOAuthTokenRefreshConfig, McpOAuthTokenResponse,
};
pub use registration::{
    validate_mcp_registration_policy, McpDiscoveryPlan, McpRegistrationError, McpRegistrationPolicy,
};
pub use stdio::{
    McpStdioEnvValue, McpStdioLaunchConfig, McpStdioLaunchError, McpStdioLaunchPlan,
    McpStdioLifecyclePhase, McpStdioLifecyclePolicy, McpStdioToolCallPlan,
};
pub use streamable_http::{
    parse_mcp_tool_call_response, McpStreamableHttpRequestPlan, McpStreamableHttpResponse,
};
pub use tool_code::mcp_tool_code;
pub use types::{
    McpAuthScope, McpAuthType, McpDiscoveredTool, McpServerStatus, McpToolDescriptor,
    McpToolInvocationRequest, McpToolInvocationResult, McpTransportKind,
};

pub const CRATE_ID: &str = "novex-mcp";
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
pub const MCP_STDIO_MIN_TIMEOUT_MS: u64 = 100;
pub const MCP_STDIO_MAX_TIMEOUT_MS: u64 = 60_000;
pub const MCP_STDIO_DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;
pub const MCP_STDIO_DEFAULT_SHUTDOWN_TIMEOUT_MS: u64 = 5_000;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "MCP Gateway",
        "ai-foundation",
        "MCP server registration, tool discovery, tenant authorization, secret, and audit boundaries.",
    )
}

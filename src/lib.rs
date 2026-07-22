#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod client;
mod error;
mod types;

pub use client::{
    ACCOUNT_ID_ENV, API_TOKEN_ENV, AccountId, ApiBaseUrl, ApiToken, ChatCompletions, Client,
    ClientBuilder, GatewayId, RequestTimeout, ResponseSizeLimit,
};
pub use error::{Error, InvalidConfiguration, ProviderErrorCode, ProviderFailure, ToolError};
pub use types::{
    AssistantMessage, ChatChoice, ChatCompletion, ChatMessage, ChatRequest, FinishReason,
    FunctionTool, MaxTokens, ModelId, Temperature, ToolCall, ToolChoice, ToolDefinition, Usage,
};

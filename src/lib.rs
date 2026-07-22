#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::missing_errors_doc, clippy::missing_panics_doc)]

mod client;
mod error;
mod types;

/// Compile-time request states, exhaustive output handling, and typed tool proofs.
///
/// This module contains documentation only. The guide below covers the state
/// graph, examples, and the boundary between compile-time and runtime validation.
#[doc = include_str!("../docs/TYPE_SYSTEM.md")]
pub mod type_system {}

pub use client::{
    ACCOUNT_ID_ENV, API_TOKEN_ENV, AccountId, ApiBaseUrl, ApiToken, ChatCompletions, Client,
    ClientBuilder, GatewayId, RequestTimeout, ResponseSizeLimit,
};
pub use error::{Error, InvalidConfiguration, ProviderErrorCode, ProviderFailure, ToolError};
pub use types::request_state;
pub use types::{
    AssistantMessage, AssistantOutput, ChatChoice, ChatCompletion, ChatMessage, ChatRequest,
    ChatRequestBuilder, FinishReason, FunctionTool, MaxTokens, ModelId, Temperature, ToolCall,
    ToolChoice, ToolDefinition, Usage, ValidatedToolCall,
};

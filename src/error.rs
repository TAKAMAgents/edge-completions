use reqwest::StatusCode;

/// Errors returned while configuring or calling the provider endpoint.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A local configuration value violated its invariant.
    #[error(transparent)]
    InvalidConfiguration(#[from] InvalidConfiguration),

    /// The HTTP exchange failed before a complete response body was received.
    #[error("provider request failed before receiving a complete response: {source}")]
    Transport {
        /// The underlying HTTP client failure.
        #[source]
        source: reqwest::Error,
    },

    /// The provider returned a non-success HTTP status.
    #[error("provider returned HTTP {status}: {failure}")]
    Provider {
        /// The HTTP status returned by the provider.
        status: StatusCode,
        /// A sanitized, typed representation of the provider failure.
        failure: ProviderFailure,
    },

    /// A success response did not match the supported typed contract.
    #[error("provider returned an invalid chat-completion response: {source}")]
    InvalidResponse {
        /// The JSON decoding failure. The response body itself is not retained.
        #[source]
        source: serde_json::Error,
    },

    /// The response body exceeded the configured safety limit.
    #[error("provider response exceeded the configured {limit_bytes}-byte limit")]
    ResponseTooLarge {
        /// Maximum number of response bytes accepted by this client.
        limit_bytes: usize,
    },

    /// A completion did not contain any choices.
    #[error("provider returned a chat completion without any choices")]
    MissingChoice,

    /// A tool-call completion did not contain a tool call.
    #[error("provider returned a tool-call completion without a tool call")]
    MissingToolCall,

    /// A final completion did not contain text content.
    #[error("provider returned a final completion without text content")]
    MissingContent,

    /// A typed tool contract was invalid or could not decode provider output.
    #[error(transparent)]
    Tool(#[from] ToolError),
}

/// A numeric error code returned by Cloudflare's API envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderErrorCode(u64);

impl ProviderErrorCode {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the provider's numeric code.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ProviderErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A sanitized failure decoded from a non-success provider response.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProviderFailure {
    /// Cloudflare returned both a stable numeric code and a message.
    #[error("{message} (provider code {code})")]
    Coded {
        /// Cloudflare's numeric error code.
        code: ProviderErrorCode,
        /// Provider message truncated to a safe maximum length.
        message: String,
    },
    /// An OpenAI-compatible error envelope contained a message.
    #[error("{message}")]
    Message {
        /// Provider message truncated to a safe maximum length.
        message: String,
    },
    /// The response did not match a supported error envelope.
    #[error("provider returned an unrecognized error response")]
    Unrecognized,
}

/// Failures raised while validating local SDK configuration.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum InvalidConfiguration {
    /// A required environment variable was absent or not valid Unicode.
    #[error("required environment variable {name} is missing or is not valid Unicode")]
    MissingEnvironmentVariable {
        /// Name of the required variable. Its value is never included.
        name: &'static str,
    },
    /// The Cloudflare account identifier was empty.
    #[error("Cloudflare account ID must not be empty")]
    EmptyAccountId,
    /// The Cloudflare API token was empty.
    #[error("Cloudflare API token must not be empty")]
    EmptyApiToken,
    /// The provider model identifier was empty.
    #[error("model ID must not be empty")]
    EmptyModelId,
    /// A chat request did not contain any messages.
    #[error("chat request must contain at least one message")]
    EmptyMessages,
    /// A tool-enabled request did not contain any tool definitions.
    #[error("a tool-enabled request must contain at least one tool definition")]
    EmptyTools,
    /// Sampling temperature was outside the supported range.
    #[error("temperature must be finite and between 0 and 2 inclusive, got {value}")]
    InvalidTemperature {
        /// Rejected temperature.
        value: f32,
    },
    /// The maximum completion-token count was zero.
    #[error("maximum token count must be greater than zero")]
    ZeroMaxTokens,
    /// The request timeout was zero.
    #[error("request timeout must be greater than zero")]
    ZeroTimeout,
    /// The response-body limit was zero.
    #[error("response-body byte limit must be greater than zero")]
    ZeroResponseSizeLimit,
    /// The optional AI Gateway identifier could not be used as a header value.
    #[error("Cloudflare AI Gateway ID is not a valid HTTP header value")]
    InvalidGatewayId,
    /// The API base URL was not a valid hierarchical URL.
    #[error("provider API base URL is invalid: {reason}")]
    InvalidBaseUrl {
        /// Safe explanation of the violated URL invariant.
        reason: String,
    },
    /// A non-TLS URL did not point to a loopback test server.
    #[error("provider API base URL must use HTTPS unless it points to a loopback host")]
    InsecureBaseUrl,
}

/// Failures while defining, decoding, or encoding a typed tool contract.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolError {
    /// A tool definition has an empty required field.
    #[error("tool definition field {field} must not be empty")]
    InvalidDefinition {
        /// Name of the invalid definition field.
        field: &'static str,
    },

    /// A model proposed a different tool from the typed contract requested by the caller.
    #[error("expected tool {expected}, but the model requested {actual}")]
    UnexpectedName {
        /// Name declared by the typed tool contract.
        expected: &'static str,
        /// Name proposed by the model.
        actual: String,
    },

    /// Model-produced JSON arguments failed typed deserialization or validation.
    #[error("tool {tool} returned invalid arguments: {source}")]
    InvalidArguments {
        /// Tool name associated with the model proposal.
        tool: String,
        /// JSON or custom deserialization failure.
        #[source]
        source: serde_json::Error,
    },

    /// A typed application result could not be encoded for the follow-up request.
    #[error("failed to encode the typed result for tool {tool}: {source}")]
    ResultEncoding {
        /// Tool name associated with the typed result.
        tool: String,
        /// JSON serialization failure.
        #[source]
        source: serde_json::Error,
    },
}

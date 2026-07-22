use std::{fmt, net::IpAddr, str::FromStr, time::Duration};

use reqwest::{Url, header};
use serde::Deserialize;
use url::Host;

use crate::{
    ChatCompletion, ChatRequest, Error, InvalidConfiguration, ProviderErrorCode, ProviderFailure,
};

const DEFAULT_BASE_URL: &str = "https://api.cloudflare.com/client/v4/";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_RESPONSE_SIZE_LIMIT: usize = 4 * 1024 * 1024;
const GATEWAY_ID_HEADER: &str = "cf-aig-gateway-id";
const LEGACY_KIMI_API_TOKEN_ENV: &str = "KIMI3_ON_CLOUDFLARE_API_KEY";

/// Environment variable read by [`Client::from_env`] for the Cloudflare account ID.
pub const ACCOUNT_ID_ENV: &str = "CLOUDFLARE_ACCOUNT_ID";

/// Environment variable read by [`Client::from_env`] for the Cloudflare API token.
pub const API_TOKEN_ENV: &str = "CLOUDFLARE_API_TOKEN";

/// Provider-independent capability boundary for chat-completion adapters.
///
/// Application code can accept `&dyn ChatCompletions` and substitute a test
/// implementation without depending on HTTP or Cloudflare configuration.
#[async_trait::async_trait]
pub trait ChatCompletions: Send + Sync {
    /// Executes one typed chat-completion request.
    async fn complete(&self, request: &ChatRequest) -> Result<ChatCompletion, Error>;
}

/// A validated Cloudflare account identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId(String);

impl AccountId {
    /// Validates and constructs an account identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidConfiguration> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(InvalidConfiguration::EmptyAccountId);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for AccountId {
    type Err = InvalidConfiguration;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<&str> for AccountId {
    type Error = InvalidConfiguration;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A validated API token whose debug output is always redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiToken(String);

impl ApiToken {
    /// Validates and constructs an API token.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidConfiguration> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(InvalidConfiguration::EmptyApiToken);
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for ApiToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiToken([REDACTED])")
    }
}

impl FromStr for ApiToken {
    type Err = InvalidConfiguration;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ApiToken {
    type Error = InvalidConfiguration;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A validated API base URL.
///
/// HTTPS is required except for loopback hosts, which are permitted so callers
/// can run local contract tests without sending credentials over a network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiBaseUrl(Url);

impl ApiBaseUrl {
    /// Parses and validates a provider API base URL.
    pub fn new(value: impl AsRef<str>) -> Result<Self, InvalidConfiguration> {
        let url =
            Url::parse(value.as_ref()).map_err(|source| InvalidConfiguration::InvalidBaseUrl {
                reason: source.to_string(),
            })?;

        if url.cannot_be_a_base() || url.host().is_none() {
            return Err(InvalidConfiguration::InvalidBaseUrl {
                reason: "URL must be hierarchical and include a host".to_owned(),
            });
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(InvalidConfiguration::InvalidBaseUrl {
                reason: "embedded credentials are not allowed".to_owned(),
            });
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(InvalidConfiguration::InvalidBaseUrl {
                reason: "query strings and fragments are not allowed".to_owned(),
            });
        }
        if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback(&url)) {
            return Err(InvalidConfiguration::InsecureBaseUrl);
        }

        Ok(Self(url))
    }

    /// Returns the validated URL.
    #[must_use]
    pub fn as_url(&self) -> &Url {
        &self.0
    }
}

impl FromStr for ApiBaseUrl {
    type Err = InvalidConfiguration;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ApiBaseUrl {
    type Error = InvalidConfiguration;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A validated request timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTimeout(Duration);

impl RequestTimeout {
    /// Creates a non-zero request timeout.
    pub fn new(value: Duration) -> Result<Self, InvalidConfiguration> {
        if value.is_zero() {
            return Err(InvalidConfiguration::ZeroTimeout);
        }
        Ok(Self(value))
    }

    /// Returns the timeout duration.
    #[must_use]
    pub fn duration(self) -> Duration {
        self.0
    }
}

impl Default for RequestTimeout {
    fn default() -> Self {
        Self(DEFAULT_TIMEOUT)
    }
}

/// A validated maximum response-body size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseSizeLimit(usize);

impl ResponseSizeLimit {
    /// Creates a non-zero response-body byte limit.
    pub fn new(bytes: usize) -> Result<Self, InvalidConfiguration> {
        if bytes == 0 {
            return Err(InvalidConfiguration::ZeroResponseSizeLimit);
        }
        Ok(Self(bytes))
    }

    /// Returns the maximum accepted number of bytes.
    #[must_use]
    pub fn bytes(self) -> usize {
        self.0
    }
}

impl Default for ResponseSizeLimit {
    fn default() -> Self {
        Self(DEFAULT_RESPONSE_SIZE_LIMIT)
    }
}

/// A validated value for Cloudflare's optional `cf-aig-gateway-id` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayId(String);

impl GatewayId {
    /// Validates and constructs an AI Gateway identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidConfiguration> {
        let value = value.into();
        if value.trim().is_empty() || header::HeaderValue::from_str(&value).is_err() {
            return Err(InvalidConfiguration::InvalidGatewayId);
        }
        Ok(Self(value))
    }

    /// Returns the validated header value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for GatewayId {
    type Err = InvalidConfiguration;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<&str> for GatewayId {
    type Error = InvalidConfiguration;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// HTTP implementation of the typed chat-completion capability.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    endpoint: Url,
    api_token: ApiToken,
    gateway_id: Option<GatewayId>,
    response_size_limit: ResponseSizeLimit,
}

impl Client {
    /// Creates a client from `CLOUDFLARE_ACCOUNT_ID` and
    /// `CLOUDFLARE_API_TOKEN`.
    pub fn from_env() -> Result<Self, Error> {
        Self::builder_from_env()?.build()
    }

    /// Starts a client builder from environment-provided credentials.
    ///
    /// This is the environment-based counterpart to [`Client::builder`]. It
    /// lets applications retain the standard credential lookup while
    /// configuring transport policy or AI Gateway routing before building the
    /// client.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidConfiguration`] when a required credential is
    /// missing or violates its typed invariant.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use edge_completions::{Client, GatewayId};
    ///
    /// # fn configured_client() -> Result<Client, edge_completions::Error> {
    /// let client = Client::builder_from_env()?
    ///     .gateway_id(GatewayId::new("production-gateway")?)
    ///     .build()?;
    /// # Ok(client)
    /// # }
    /// ```
    pub fn builder_from_env() -> Result<ClientBuilder, Error> {
        Self::builder_from_env_with(|name| std::env::var(name))
    }

    /// Creates a client from validated credentials using default transport settings.
    pub fn new(account_id: AccountId, api_token: ApiToken) -> Result<Self, Error> {
        Self::builder(account_id, api_token).build()
    }

    /// Starts a client builder from validated credentials.
    #[must_use]
    pub fn builder(account_id: AccountId, api_token: ApiToken) -> ClientBuilder {
        ClientBuilder::new(account_id, api_token)
    }

    /// Executes one typed chat-completion request.
    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatCompletion, Error> {
        let mut call = self
            .http
            .post(self.endpoint.clone())
            .bearer_auth(&self.api_token.0)
            .json(request);

        if let Some(gateway_id) = &self.gateway_id {
            call = call.header(GATEWAY_ID_HEADER, gateway_id.as_str());
        }

        let response = call
            .send()
            .await
            .map_err(|source| Error::Transport { source })?;
        let status = response.status();
        let body = read_bounded_body(response, self.response_size_limit).await?;

        if !status.is_success() {
            return Err(Error::Provider {
                status,
                failure: provider_failure(&body),
            });
        }

        serde_json::from_slice(&body).map_err(|source| Error::InvalidResponse { source })
    }

    fn builder_from_env_with(
        read: impl Fn(&str) -> Result<String, std::env::VarError>,
    ) -> Result<ClientBuilder, Error> {
        let account_id =
            read(ACCOUNT_ID_ENV).map_err(|_| InvalidConfiguration::MissingEnvironmentVariable {
                name: ACCOUNT_ID_ENV,
            })?;
        let api_token = match read(API_TOKEN_ENV) {
            Ok(value) => value,
            Err(_) => read(LEGACY_KIMI_API_TOKEN_ENV).map_err(|_| {
                InvalidConfiguration::MissingEnvironmentVariable {
                    name: API_TOKEN_ENV,
                }
            })?,
        };
        Ok(Self::builder(
            AccountId::new(account_id)?,
            ApiToken::new(api_token)?,
        ))
    }
}

#[async_trait::async_trait]
impl ChatCompletions for Client {
    async fn complete(&self, request: &ChatRequest) -> Result<ChatCompletion, Error> {
        self.chat(request).await
    }
}

/// Builder for transport policy and optional Cloudflare AI Gateway routing.
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    account_id: AccountId,
    api_token: ApiToken,
    base_url: Option<ApiBaseUrl>,
    timeout: RequestTimeout,
    gateway_id: Option<GatewayId>,
    response_size_limit: ResponseSizeLimit,
}

impl ClientBuilder {
    fn new(account_id: AccountId, api_token: ApiToken) -> Self {
        Self {
            account_id,
            api_token,
            base_url: None,
            timeout: RequestTimeout::default(),
            gateway_id: None,
            response_size_limit: ResponseSizeLimit::default(),
        }
    }

    /// Overrides the API base URL.
    #[must_use]
    pub fn base_url(mut self, base_url: ApiBaseUrl) -> Self {
        self.base_url = Some(base_url);
        self
    }

    /// Overrides the default 60-second request timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: RequestTimeout) -> Self {
        self.timeout = timeout;
        self
    }

    /// Adds Cloudflare's optional AI Gateway routing header.
    #[must_use]
    pub fn gateway_id(mut self, gateway_id: GatewayId) -> Self {
        self.gateway_id = Some(gateway_id);
        self
    }

    /// Overrides the default four-megabyte response-body limit.
    #[must_use]
    pub fn response_size_limit(mut self, limit: ResponseSizeLimit) -> Self {
        self.response_size_limit = limit;
        self
    }

    /// Validates transport configuration and creates the client.
    pub fn build(self) -> Result<Client, Error> {
        let base_url = match self.base_url {
            Some(base_url) => base_url,
            None => ApiBaseUrl::new(DEFAULT_BASE_URL)?,
        };
        let mut endpoint = base_url.0;
        endpoint
            .path_segments_mut()
            .map_err(|_| InvalidConfiguration::InvalidBaseUrl {
                reason: "URL cannot be used as a hierarchical API base".to_owned(),
            })?
            .pop_if_empty()
            .extend([
                "accounts",
                self.account_id.as_str(),
                "ai",
                "v1",
                "chat",
                "completions",
            ]);

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(concat!(
                "edge-completions/",
                env!("CARGO_PKG_VERSION")
            )),
        );
        let http = reqwest::Client::builder()
            .timeout(self.timeout.duration())
            .default_headers(headers)
            .build()
            .map_err(|source| Error::Transport { source })?;

        Ok(Client {
            http,
            endpoint,
            api_token: self.api_token,
            gateway_id: self.gateway_id,
            response_size_limit: self.response_size_limit,
        })
    }
}

async fn read_bounded_body(
    mut response: reqwest::Response,
    limit: ResponseSizeLimit,
) -> Result<Vec<u8>, Error> {
    let limit_bytes = limit.bytes();
    if response
        .content_length()
        .is_some_and(|length| length > limit_bytes as u64)
    {
        return Err(Error::ResponseTooLarge { limit_bytes });
    }

    let declared_capacity = match response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
    {
        Some(length) => length.min(limit_bytes),
        None => 0,
    };
    let mut body = Vec::with_capacity(declared_capacity);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|source| Error::Transport { source })?
    {
        if body.len().saturating_add(chunk.len()) > limit_bytes {
            return Err(Error::ResponseTooLarge { limit_bytes });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => IpAddr::V4(address).is_loopback(),
        Some(Host::Ipv6(address)) => IpAddr::V6(address).is_loopback(),
        None => false,
    }
}

#[derive(Deserialize)]
struct OpenAiErrorEnvelope {
    error: OpenAiError,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

#[derive(Deserialize)]
struct CloudflareErrorEnvelope {
    errors: Vec<CloudflareApiError>,
}

#[derive(Deserialize)]
struct CloudflareApiError {
    code: u64,
    message: String,
}

fn provider_failure(body: &[u8]) -> ProviderFailure {
    if let Ok(envelope) = serde_json::from_slice::<CloudflareErrorEnvelope>(body) {
        if let Some(error) = envelope.errors.into_iter().next() {
            return ProviderFailure::Coded {
                code: ProviderErrorCode::new(error.code),
                message: bounded_message(error.message),
            };
        }
    }
    match serde_json::from_slice::<OpenAiErrorEnvelope>(body) {
        Ok(envelope) => ProviderFailure::Message {
            message: bounded_message(envelope.error.message),
        },
        Err(_) => ProviderFailure::Unrecognized,
    }
}

fn bounded_message(message: String) -> String {
    message.chars().take(1_000).collect()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, env::VarError};

    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header, method, path},
    };

    use super::*;
    use crate::ChatMessage;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn credentials() -> Result<(AccountId, ApiToken), InvalidConfiguration> {
        Ok((AccountId::new("account-1")?, ApiToken::new("secret-token")?))
    }

    #[test]
    fn creates_client_from_named_environment_variables() -> TestResult {
        let values = HashMap::from([
            (ACCOUNT_ID_ENV, "account-1"),
            (API_TOKEN_ENV, "super-secret"),
        ]);
        let client = Client::builder_from_env_with(|name| {
            values
                .get(name)
                .map(ToString::to_string)
                .ok_or(VarError::NotPresent)
        })?
        .build()?;

        assert!(!format!("{client:?}").contains("super-secret"));
        assert_eq!(
            client.endpoint.as_str(),
            "https://api.cloudflare.com/client/v4/accounts/account-1/ai/v1/chat/completions"
        );
        Ok(())
    }

    #[test]
    fn reports_the_missing_environment_variable_by_name() -> TestResult {
        let error = match Client::builder_from_env_with(|_| Err(VarError::NotPresent)) {
            Err(error) => error,
            Ok(_) => return Err("client creation unexpectedly succeeded".into()),
        };

        assert_eq!(
            error.to_string(),
            "required environment variable CLOUDFLARE_ACCOUNT_ID is missing or is not valid Unicode"
        );
        Ok(())
    }

    #[test]
    fn reports_a_missing_api_token_without_reading_or_displaying_it() -> TestResult {
        let result = Client::builder_from_env_with(|name| match name {
            ACCOUNT_ID_ENV => Ok("account-1".to_owned()),
            _ => Err(VarError::NotPresent),
        });
        let error = match result {
            Err(error) => error,
            Ok(_) => return Err("client creation unexpectedly succeeded".into()),
        };

        assert_eq!(
            error.to_string(),
            "required environment variable CLOUDFLARE_API_TOKEN is missing or is not valid Unicode"
        );
        Ok(())
    }

    #[test]
    fn supports_the_original_kimi_token_variable_as_a_compatibility_fallback() -> TestResult {
        let values = HashMap::from([
            (ACCOUNT_ID_ENV, "account-1"),
            (LEGACY_KIMI_API_TOKEN_ENV, "legacy-secret"),
        ]);
        let client = Client::builder_from_env_with(|name| {
            values
                .get(name)
                .map(ToString::to_string)
                .ok_or(VarError::NotPresent)
        })?
        .build()?;

        assert!(!format!("{client:?}").contains("legacy-secret"));
        Ok(())
    }

    #[test]
    fn rejects_non_loopback_plain_http_base_urls() {
        assert!(matches!(
            ApiBaseUrl::new("http://example.com/client/v4/"),
            Err(InvalidConfiguration::InsecureBaseUrl)
        ));
    }

    #[tokio::test]
    async fn sends_request_and_decodes_tool_call() -> TestResult {
        let server = MockServer::start().await;
        let request = ChatRequest::kimi_k3(vec![ChatMessage::user("weather?")])?;
        Mock::given(method("POST"))
            .and(path("/accounts/account-1/ai/v1/chat/completions"))
            .and(header("authorization", "Bearer secret-token"))
            .and(header(
                "user-agent",
                concat!("edge-completions/", env!("CARGO_PKG_VERSION")),
            ))
            .and(body_json(json!({"model":"moonshotai/kimi-k3","messages":[{"role":"user","content":"weather?"}]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"chatcmpl-1","object":"chat.completion","created":1,"model":"moonshotai/kimi-k3",
                "choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"city\":\"Paris\"}"}}]},"finish_reason":"tool_calls"}],
                "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}
            })))
            .mount(&server)
            .await;

        let (account_id, token) = credentials()?;
        let client = Client::builder(account_id, token)
            .base_url(ApiBaseUrl::new(format!("{}/", server.uri()))?)
            .build()?;
        let response = client.chat(&request).await?;
        let choice = response.first_choice()?;
        let call = choice
            .message()
            .tool_calls()
            .first()
            .ok_or("response did not contain the expected tool call")?;

        assert_eq!(
            choice.finish_reason(),
            Some(&crate::FinishReason::ToolCalls)
        );
        assert_eq!(call.name(), "get_weather");
        Ok(())
    }

    #[tokio::test]
    async fn maps_provider_error_without_exposing_token() -> TestResult {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(json!({"error":{"message":"invalid token"}})),
            )
            .mount(&server)
            .await;
        let client = Client::builder(AccountId::new("account-1")?, ApiToken::new("super-secret")?)
            .base_url(ApiBaseUrl::new(format!("{}/", server.uri()))?)
            .build()?;
        let request = ChatRequest::kimi_k3(vec![ChatMessage::user("hi")])?;

        let error = match client.chat(&request).await {
            Err(error) => error,
            Ok(_) => return Err("request unexpectedly succeeded".into()),
        };
        assert_eq!(
            error.to_string(),
            "provider returned HTTP 401 Unauthorized: invalid token"
        );
        assert!(!format!("{client:?}").contains("super-secret"));
        Ok(())
    }

    #[tokio::test]
    async fn maps_cloudflare_error_envelope_without_exposing_raw_json() -> TestResult {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(402).set_body_json(json!({
                "errors": [{
                    "message": "Insufficient balance; add money to your gateway or use BYOK",
                    "code": 2021
                }],
                "success": false,
                "result": {},
                "messages": []
            })))
            .mount(&server)
            .await;
        let client = Client::builder(AccountId::new("account-1")?, ApiToken::new("super-secret")?)
            .base_url(ApiBaseUrl::new(format!("{}/", server.uri()))?)
            .build()?;
        let request = ChatRequest::kimi_k3(vec![ChatMessage::user("hi")])?;

        let error = match client.chat(&request).await {
            Err(error) => error,
            Ok(_) => return Err("request unexpectedly succeeded".into()),
        };
        assert_eq!(
            error.to_string(),
            "provider returned HTTP 402 Payment Required: Insufficient balance; add money to your gateway or use BYOK (provider code 2021)"
        );
        assert!(!error.to_string().contains("\"errors\""));
        Ok(())
    }

    #[tokio::test]
    async fn rejects_a_response_body_larger_than_the_configured_limit() -> TestResult {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(65)))
            .mount(&server)
            .await;
        let (account_id, token) = credentials()?;
        let client = Client::builder(account_id, token)
            .base_url(ApiBaseUrl::new(format!("{}/", server.uri()))?)
            .response_size_limit(ResponseSizeLimit::new(64)?)
            .build()?;
        let request = ChatRequest::kimi_k3(vec![ChatMessage::user("hi")])?;

        assert!(matches!(
            client.chat(&request).await,
            Err(Error::ResponseTooLarge { limit_bytes: 64 })
        ));
        Ok(())
    }
}

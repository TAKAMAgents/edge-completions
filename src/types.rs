use std::{fmt, marker::PhantomData, str::FromStr};

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{Error, InvalidConfiguration, ToolError};

/// A validated provider model identifier, such as `moonshotai/kimi-k3`.
///
/// ```
/// use edge_completions::ModelId;
///
/// let model = ModelId::new("moonshotai/kimi-k3")?;
/// assert_eq!(model.as_str(), "moonshotai/kimi-k3");
/// # Ok::<(), edge_completions::InvalidConfiguration>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ModelId(String);

impl ModelId {
    /// Returns the supported Kimi K3 model identifier.
    #[must_use]
    pub fn kimi_k3() -> Self {
        Self("moonshotai/kimi-k3".to_owned())
    }

    /// Validates and constructs a model identifier.
    ///
    /// Leading and trailing whitespace is removed before storage.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidConfiguration::EmptyModelId`] when the trimmed value is
    /// empty.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidConfiguration> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(InvalidConfiguration::EmptyModelId);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated provider model identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ModelId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

impl TryFrom<&str> for ModelId {
    type Error = InvalidConfiguration;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for ModelId {
    type Err = InvalidConfiguration;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// A typed chat message accepted by the provider request contract.
///
/// Use the role-specific constructors instead of constructing provider payloads
/// directly:
///
/// ```
/// use edge_completions::ChatMessage;
///
/// let message = ChatMessage::user("Explain trait boundaries.");
/// assert_eq!(message, ChatMessage::user("Explain trait boundaries."));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChatMessage(MessagePayload);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum MessagePayload {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: ToolCallId,
        content: String,
    },
}

impl ChatMessage {
    /// Creates a system instruction message.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self(MessagePayload::System {
            content: content.into(),
        })
    }

    /// Creates a user message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self(MessagePayload::User {
            content: content.into(),
        })
    }

    /// Creates an assistant text message.
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self(MessagePayload::Assistant {
            content: Some(content.into()),
            tool_calls: Vec::new(),
        })
    }

    /// Replays provider-proposed tool calls in a follow-up request.
    #[must_use]
    pub fn assistant_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self(MessagePayload::Assistant {
            content: None,
            tool_calls,
        })
    }

    /// Encodes a typed tool result after confirming that the call name matches `T`.
    ///
    /// Prefer [`ValidatedToolCall::result`] after calling
    /// [`ToolCall::validate`]. That path carries the successful name and
    /// argument validation in the type system.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::UnexpectedName`] when the proposed function name
    /// does not match `T::NAME`, or [`ToolError::ResultEncoding`] when the typed
    /// output cannot be serialized.
    pub fn tool_result<T: ToolDefinition>(
        call: &ToolCall,
        result: &T::Output,
    ) -> Result<Self, ToolError> {
        call.ensure_name::<T>()?;
        let content =
            serde_json::to_string(result).map_err(|source| ToolError::ResultEncoding {
                tool: call.name().to_owned(),
                source,
            })?;
        Ok(Self(MessagePayload::Tool {
            tool_call_id: call.id.clone(),
            content,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
struct ToolCallId(String);

impl<'de> Deserialize<'de> for ToolCallId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(de::Error::custom("tool-call ID must not be empty"));
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct FunctionDefinition {
    name: String,
    description: String,
    parameters: Schema,
}

impl FunctionDefinition {
    fn new(name: impl Into<String>, description: impl Into<String>, parameters: Schema) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// A typed tool contract used for schema generation, argument decoding, and result encoding.
///
/// The associated types ensure that one tool name, one validated argument type,
/// and one result type travel together through the SDK.
///
/// ```
/// use edge_completions::ToolDefinition;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// struct LookupWeather;
///
/// #[derive(Deserialize, JsonSchema)]
/// struct Arguments { city: String }
///
/// #[derive(Serialize)]
/// struct Report { temperature_celsius: i16 }
///
/// impl ToolDefinition for LookupWeather {
///     type Arguments = Arguments;
///     type Output = Report;
///     const NAME: &'static str = "lookup_weather";
///     const DESCRIPTION: &'static str = "Look up the weather for a city";
/// }
///
/// assert_eq!(LookupWeather::NAME, "lookup_weather");
/// ```
pub trait ToolDefinition {
    /// Validated application type decoded from model-produced JSON arguments.
    type Arguments: serde::de::DeserializeOwned + JsonSchema;

    /// Application type serialized into the tool-result message.
    type Output: Serialize;

    /// Stable function name exposed to the model.
    const NAME: &'static str;

    /// Clear function description exposed to the model.
    const DESCRIPTION: &'static str;
}

/// A validated OpenAI-compatible function-tool definition.
///
/// ```
/// # use edge_completions::{FunctionTool, ToolDefinition};
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// # struct LookupWeather;
/// # #[derive(Deserialize, JsonSchema)] struct Arguments { city: String }
/// # #[derive(Serialize)] struct Report { temperature_celsius: i16 }
/// # impl ToolDefinition for LookupWeather {
/// #     type Arguments = Arguments;
/// #     type Output = Report;
/// #     const NAME: &'static str = "lookup_weather";
/// #     const DESCRIPTION: &'static str = "Look up the weather for a city";
/// # }
/// let tool = FunctionTool::for_tool::<LookupWeather>()?;
/// assert_eq!(tool.name(), "lookup_weather");
/// # Ok::<(), edge_completions::ToolError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionTool {
    #[serde(rename = "type")]
    kind: FunctionToolKind,
    function: FunctionDefinition,
}

impl FunctionTool {
    /// Generates a function definition from the typed contract `T`.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::InvalidDefinition`] when `T::NAME` or
    /// `T::DESCRIPTION` is empty after trimming.
    pub fn for_tool<T: ToolDefinition>() -> Result<Self, ToolError> {
        if T::NAME.trim().is_empty() {
            return Err(ToolError::InvalidDefinition { field: "name" });
        }
        if T::DESCRIPTION.trim().is_empty() {
            return Err(ToolError::InvalidDefinition {
                field: "description",
            });
        }
        Ok(Self {
            kind: FunctionToolKind::Function,
            function: FunctionDefinition::new(
                T::NAME,
                T::DESCRIPTION,
                schemars::schema_for!(T::Arguments),
            ),
        })
    }

    /// Returns the function name exposed by this definition.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.function.name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FunctionToolKind {
    Function,
}

/// Sampling temperature constrained to the provider's documented `[0, 2]` range.
///
/// ```
/// use edge_completions::Temperature;
///
/// let temperature = Temperature::new(0.2)?;
/// assert_eq!(temperature.value(), 0.2);
/// # Ok::<(), edge_completions::InvalidConfiguration>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Temperature(f32);

impl Temperature {
    /// Creates a temperature in the inclusive range from zero to two.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidConfiguration::InvalidTemperature`] when `value` is not
    /// finite or falls outside the inclusive range `0.0..=2.0`.
    pub fn new(value: f32) -> Result<Self, InvalidConfiguration> {
        if value.is_finite() && (0.0..=2.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidConfiguration::InvalidTemperature { value })
        }
    }

    /// Returns the validated temperature.
    #[must_use]
    pub fn value(self) -> f32 {
        self.0
    }
}

/// A validated, non-zero completion-token limit.
///
/// ```
/// use edge_completions::MaxTokens;
///
/// let limit = MaxTokens::new(256)?;
/// assert_eq!(limit.value(), 256);
/// # Ok::<(), edge_completions::InvalidConfiguration>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct MaxTokens(u32);

impl MaxTokens {
    /// Creates a non-zero completion-token limit.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidConfiguration::ZeroMaxTokens`] when `value` is zero.
    pub fn new(value: u32) -> Result<Self, InvalidConfiguration> {
        if value == 0 {
            Err(InvalidConfiguration::ZeroMaxTokens)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the validated token limit.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

/// Provider setting for selecting function tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ToolChoice {
    /// Do not let the model propose a tool call.
    None,
    /// Let the model choose whether to propose a tool call.
    Auto,
    /// Require the model to propose a tool call.
    Required,
}

/// Compile-time states used by [`ChatRequestBuilder`].
///
/// The traits are sealed: callers can name the states in generic APIs, but
/// only this crate can define new legal request states.
pub mod request_state {
    mod sealed {
        pub trait Sealed {}
    }

    /// A sealed witness for the message-cardinality state of a request builder.
    pub trait MessageState: sealed::Sealed {}

    /// A sealed witness for the tool-cardinality state of a request builder.
    pub trait ToolState: sealed::Sealed {}

    /// Witness that a request builder does not yet contain a message.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NeedsMessage;

    /// Witness that a request builder contains at least one message.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct HasMessages;

    /// Witness that a request builder does not contain a tool definition.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WithoutTools;

    /// Witness that a request builder contains at least one tool definition.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WithTools;

    impl sealed::Sealed for NeedsMessage {}
    impl sealed::Sealed for HasMessages {}
    impl sealed::Sealed for WithoutTools {}
    impl sealed::Sealed for WithTools {}

    impl MessageState for NeedsMessage {}
    impl MessageState for HasMessages {}
    impl ToolState for WithoutTools {}
    impl ToolState for WithTools {}
}

use request_state::{HasMessages, MessageState, NeedsMessage, ToolState, WithTools, WithoutTools};

/// A typestate builder that makes invalid request-construction sequences fail
/// to compile.
///
/// `M` witnesses whether the builder has a message, and `T` witnesses whether
/// it has a tool. Methods consume one state and return the next legal state.
/// In categorical terms, the state types are objects and the available methods
/// are composable morphisms; Rust's trait bounds reject illegal compositions.
///
/// # Example
///
/// ```
/// use edge_completions::{ChatMessage, ChatRequest};
///
/// let request = ChatRequest::kimi_k3_builder()
///     .message(ChatMessage::user("Explain typestate briefly."))
///     .build();
///
/// assert_eq!(request.model().as_str(), "moonshotai/kimi-k3");
/// ```
///
/// # Compile-time guarantees
///
/// A request without a message has no `build` method:
///
/// ```compile_fail
/// use edge_completions::ChatRequest;
///
/// let _request = ChatRequest::kimi_k3_builder().build();
/// ```
///
/// A tool choice cannot be selected before a tool exists:
///
/// ```compile_fail
/// use edge_completions::{ChatRequest, ToolChoice};
///
/// let _builder = ChatRequest::kimi_k3_builder()
///     .tool_choice(ToolChoice::Required);
/// ```
#[must_use = "request builders must be transitioned and built"]
#[derive(Debug, Clone)]
pub struct ChatRequestBuilder<M: MessageState, T: ToolState> {
    model: ModelId,
    messages: Vec<ChatMessage>,
    tools: Vec<FunctionTool>,
    tool_choice: Option<ToolChoice>,
    temperature: Option<Temperature>,
    max_tokens: Option<MaxTokens>,
    state: PhantomData<(M, T)>,
}

impl<M: MessageState, T: ToolState> ChatRequestBuilder<M, T> {
    fn transition<NextM: MessageState, NextT: ToolState>(self) -> ChatRequestBuilder<NextM, NextT> {
        ChatRequestBuilder {
            model: self.model,
            messages: self.messages,
            tools: self.tools,
            tool_choice: self.tool_choice,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            state: PhantomData,
        }
    }

    /// Sets a validated sampling temperature without changing either state witness.
    pub fn temperature(mut self, temperature: Temperature) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets a validated completion-token limit without changing either state witness.
    pub fn max_tokens(mut self, max_tokens: MaxTokens) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

impl<T: ToolState> ChatRequestBuilder<NeedsMessage, T> {
    /// Adds the first message and returns the [`HasMessages`] witness state.
    pub fn message(mut self, message: ChatMessage) -> ChatRequestBuilder<HasMessages, T> {
        self.messages.push(message);
        self.transition()
    }
}

impl<T: ToolState> ChatRequestBuilder<HasMessages, T> {
    /// Appends another message while preserving the [`HasMessages`] witness.
    pub fn message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// Builds a request after the [`HasMessages`] witness proves that it is non-empty.
    #[must_use]
    pub fn build(self) -> ChatRequest {
        ChatRequest {
            model: self.model,
            messages: self.messages,
            tools: self.tools,
            tool_choice: self.tool_choice,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        }
    }
}

impl<M: MessageState> ChatRequestBuilder<M, WithoutTools> {
    /// Adds the first tool and returns the [`WithTools`] witness state.
    ///
    /// Tool selection defaults to [`ToolChoice::Auto`] and can be changed only
    /// after this transition.
    pub fn tool(mut self, tool: FunctionTool) -> ChatRequestBuilder<M, WithTools> {
        self.tools.push(tool);
        self.tool_choice = Some(ToolChoice::Auto);
        self.transition()
    }
}

impl<M: MessageState> ChatRequestBuilder<M, WithTools> {
    /// Appends another tool while preserving the [`WithTools`] witness.
    pub fn tool(mut self, tool: FunctionTool) -> Self {
        self.tools.push(tool);
        self
    }

    /// Selects the provider's tool-choice setting after at least one tool exists.
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }
}

/// A validated, serializable chat-completion request.
///
/// Prefer [`ChatRequest::builder`] or [`ChatRequest::kimi_k3_builder`] for new
/// code. Their state parameters make an empty request impossible to build.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatRequest {
    model: ModelId,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<FunctionTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<Temperature>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<MaxTokens>,
}

impl ChatRequest {
    /// Starts a typestate builder for a validated model.
    pub fn builder(model: ModelId) -> ChatRequestBuilder<NeedsMessage, WithoutTools> {
        ChatRequestBuilder {
            model,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            temperature: None,
            max_tokens: None,
            state: PhantomData,
        }
    }

    /// Starts a typestate builder for `moonshotai/kimi-k3`.
    pub fn kimi_k3_builder() -> ChatRequestBuilder<NeedsMessage, WithoutTools> {
        Self::builder(ModelId::kimi_k3())
    }

    /// Creates a request for `moonshotai/kimi-k3` with at least one message.
    ///
    /// This compatibility constructor performs the message-cardinality check at
    /// runtime. New code can use [`ChatRequest::kimi_k3_builder`] to move the
    /// same invariant to compile time.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidConfiguration::EmptyMessages`] when `messages` is empty.
    pub fn kimi_k3(messages: Vec<ChatMessage>) -> Result<Self, InvalidConfiguration> {
        Self::new(ModelId::kimi_k3(), messages)
    }

    /// Creates a request for a validated model with at least one message.
    ///
    /// This compatibility constructor performs the message-cardinality check at
    /// runtime. New code can use [`ChatRequest::builder`] instead.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidConfiguration::EmptyMessages`] when `messages` is empty.
    pub fn new(model: ModelId, messages: Vec<ChatMessage>) -> Result<Self, InvalidConfiguration> {
        if messages.is_empty() {
            return Err(InvalidConfiguration::EmptyMessages);
        }
        Ok(Self {
            model,
            messages,
            tools: Vec::new(),
            tool_choice: None,
            temperature: None,
            max_tokens: None,
        })
    }

    /// Enables one typed function tool using the selected tool choice.
    #[must_use]
    pub fn with_tool(mut self, tool: FunctionTool, choice: ToolChoice) -> Self {
        self.tools = vec![tool];
        self.tool_choice = Some(choice);
        self
    }

    /// Enables one or more typed function tools using the selected tool choice.
    ///
    /// This compatibility method validates tool cardinality at runtime. The
    /// typestate builder makes [`ChatRequestBuilder::tool_choice`] available only
    /// after at least one call to [`ChatRequestBuilder::tool`].
    ///
    /// # Errors
    ///
    /// Returns [`InvalidConfiguration::EmptyTools`] when `tools` is empty.
    pub fn with_tools(
        mut self,
        tools: Vec<FunctionTool>,
        choice: ToolChoice,
    ) -> Result<Self, InvalidConfiguration> {
        if tools.is_empty() {
            return Err(InvalidConfiguration::EmptyTools);
        }
        self.tools = tools;
        self.tool_choice = Some(choice);
        Ok(self)
    }

    /// Sets a validated sampling temperature.
    #[must_use]
    pub fn with_temperature(mut self, temperature: Temperature) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets a validated completion-token limit.
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: MaxTokens) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Returns the selected model.
    #[must_use]
    pub fn model(&self) -> &ModelId {
        &self.model
    }
}

/// A typed chat-completion response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChatCompletion {
    id: CompletionId,
    object: CompletionObject,
    created: u64,
    model: ModelId,
    choices: Vec<ChatChoice>,
    usage: Option<Usage>,
}

impl ChatCompletion {
    /// Returns the provider completion identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id.0
    }

    /// Returns the model reported by the provider.
    #[must_use]
    pub fn model(&self) -> &ModelId {
        &self.model
    }

    /// Returns the provider's Unix creation timestamp in seconds.
    #[must_use]
    pub fn created_unix_seconds(&self) -> u64 {
        self.created
    }

    /// Returns all completion choices.
    #[must_use]
    pub fn choices(&self) -> &[ChatChoice] {
        &self.choices
    }

    /// Returns the first choice or a typed missing-choice error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingChoice`] when the provider response contains no
    /// choices.
    pub fn first_choice(&self) -> Result<&ChatChoice, Error> {
        self.choices.first().ok_or(Error::MissingChoice)
    }

    /// Returns token usage when the provider supplied it.
    #[must_use]
    pub fn usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletionId(String);

impl<'de> Deserialize<'de> for CompletionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(de::Error::custom("completion ID must not be empty"));
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum CompletionObject {
    #[serde(rename = "chat.completion")]
    ChatCompletion,
}

/// One provider-generated completion choice.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChatChoice {
    index: u32,
    message: AssistantMessage,
    finish_reason: Option<FinishReason>,
}

impl ChatChoice {
    /// Returns this choice's provider index.
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Returns the assistant message.
    #[must_use]
    pub fn message(&self) -> &AssistantMessage {
        &self.message
    }

    /// Returns the typed finish reason when supplied by the provider.
    #[must_use]
    pub fn finish_reason(&self) -> Option<&FinishReason> {
        self.finish_reason.as_ref()
    }
}

/// A typed assistant message returned by the provider.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssistantMessage {
    role: AssistantRole,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

/// Exhaustive alternatives carried by a typed assistant message.
///
/// This enum is a sum type (a coproduct): callers must select one branch for
/// every supported provider outcome. `TextAndToolCalls` preserves providers
/// that return both values rather than silently discarding either one.
///
/// ```
/// use edge_completions::{AssistantMessage, AssistantOutput};
///
/// fn output_kind(message: &AssistantMessage) -> &'static str {
///     match message.output() {
///         AssistantOutput::Text(_) => "text",
///         AssistantOutput::ToolCalls(_) => "tools",
///         AssistantOutput::TextAndToolCalls { .. } => "text-and-tool-calls",
///         AssistantOutput::Empty => "empty",
///     }
/// }
/// ```
#[must_use = "assistant output should be handled explicitly"]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssistantOutput<'a> {
    /// The assistant returned final text without tool calls.
    Text(&'a str),
    /// The assistant proposed tool calls without text.
    ToolCalls(&'a [ToolCall]),
    /// The assistant returned text and proposed one or more tool calls.
    TextAndToolCalls {
        /// Provider-generated assistant text.
        text: &'a str,
        /// Untrusted tool calls that still require typed validation.
        tool_calls: &'a [ToolCall],
    },
    /// The assistant returned neither text nor tool calls.
    Empty,
}

impl AssistantMessage {
    /// Classifies assistant output into an exhaustive typed alternative.
    pub fn output(&self) -> AssistantOutput<'_> {
        match (self.content.as_deref(), self.tool_calls.as_slice()) {
            (Some(text), []) => AssistantOutput::Text(text),
            (None, []) => AssistantOutput::Empty,
            (None, tool_calls) => AssistantOutput::ToolCalls(tool_calls),
            (Some(text), tool_calls) => AssistantOutput::TextAndToolCalls { text, tool_calls },
        }
    }

    /// Returns text content when the assistant produced a final answer.
    #[must_use]
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    /// Returns all tool calls proposed by the model.
    #[must_use]
    pub fn tool_calls(&self) -> &[ToolCall] {
        &self.tool_calls
    }

    /// Returns the first proposed tool call or a typed missing-call error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingToolCall`] when the assistant proposed no tool
    /// calls.
    pub fn first_tool_call(&self) -> Result<&ToolCall, Error> {
        self.tool_calls.first().ok_or(Error::MissingToolCall)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AssistantRole {
    Assistant,
}

/// A function-tool call proposed by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    id: ToolCallId,
    #[serde(rename = "type")]
    kind: ToolCallKind,
    function: ToolCallFunction,
}

/// Proof that a model-produced tool call matched and decoded through `T`.
///
/// The witness binds the validated arguments and any encoded result to the
/// same [`ToolDefinition`]. Construct it only through [`ToolCall::validate`].
///
/// A validated call gives application code typed arguments and accepts only the
/// matching output type:
///
/// ```
/// use edge_completions::{ChatMessage, ToolCall, ToolDefinition, ToolError};
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// struct LookupWeather;
/// #[derive(Deserialize, JsonSchema)]
/// struct Arguments { city: String }
/// #[derive(Serialize)]
/// struct Report { temperature_celsius: i16 }
///
/// impl ToolDefinition for LookupWeather {
///     type Arguments = Arguments;
///     type Output = Report;
///     const NAME: &'static str = "lookup_weather";
///     const DESCRIPTION: &'static str = "Look up weather";
/// }
///
/// fn result_for(call: &ToolCall) -> Result<ChatMessage, ToolError> {
///     let validated = call.validate::<LookupWeather>()?;
///     assert!(!validated.arguments().city.is_empty());
///     validated.result(&Report { temperature_celsius: 18 })
/// }
/// ```
///
/// Passing a different output type fails to compile:
///
/// ```compile_fail
/// use edge_completions::{ToolCall, ToolDefinition, ToolError};
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// struct LookupWeather;
/// #[derive(Deserialize, JsonSchema)]
/// struct Arguments { city: String }
/// #[derive(Serialize)]
/// struct WeatherReport { temperature_celsius: i16 }
///
/// impl ToolDefinition for LookupWeather {
///     type Arguments = Arguments;
///     type Output = WeatherReport;
///     const NAME: &'static str = "lookup_weather";
///     const DESCRIPTION: &'static str = "Look up weather";
/// }
///
/// fn invalid_result(call: &ToolCall) -> Result<(), ToolError> {
///     let validated = call.validate::<LookupWeather>()?;
///     validated.result(&String::from("wrong output type"))?;
///     Ok(())
/// }
/// ```
#[must_use = "validated tool calls should be inspected or converted into results"]
pub struct ValidatedToolCall<'a, T: ToolDefinition> {
    call: &'a ToolCall,
    arguments: T::Arguments,
    tool: PhantomData<T>,
}

impl<T: ToolDefinition> ValidatedToolCall<'_, T> {
    /// Returns the validated arguments associated with `T`.
    #[must_use]
    pub fn arguments(&self) -> &T::Arguments {
        &self.arguments
    }

    /// Consumes the proof and returns its validated arguments.
    #[must_use]
    pub fn into_arguments(self) -> T::Arguments {
        self.arguments
    }

    /// Encodes an output whose type is associated with the same tool proof.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::ResultEncoding`] when `result` cannot be serialized.
    /// A name mismatch is unrepresentable here because this witness is created
    /// only after [`ToolCall::validate`] succeeds for the same `T`.
    pub fn result(&self, result: &T::Output) -> Result<ChatMessage, ToolError> {
        ChatMessage::tool_result::<T>(self.call, result)
    }

    /// Returns the underlying provider tool-call identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        self.call.id()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ToolCallKind {
    Function,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ToolCallFunction {
    name: String,
    // The provider encodes JSON arguments inside a string.
    arguments: String,
}

impl ToolCall {
    /// Returns the provider's tool-call identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id.0
    }

    /// Returns the untrusted function name proposed by the model.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.function.name
    }

    /// Validates the proposed name and arguments, returning a proof bound to
    /// the matching tool contract.
    ///
    /// This is a runtime trust-boundary check: tool calls come from the model and
    /// cannot be proven valid at compile time. After validation, the returned
    /// witness carries the established relationship between `T::Arguments` and
    /// `T::Output`.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::UnexpectedName`] when the model proposed a different
    /// function, or [`ToolError::InvalidArguments`] when the arguments cannot be
    /// decoded into `T::Arguments`.
    pub fn validate<T: ToolDefinition>(&self) -> Result<ValidatedToolCall<'_, T>, ToolError> {
        self.ensure_name::<T>()?;
        let arguments = serde_json::from_str(&self.function.arguments).map_err(|source| {
            ToolError::InvalidArguments {
                tool: self.name().to_owned(),
                source,
            }
        })?;
        Ok(ValidatedToolCall {
            call: self,
            arguments,
            tool: PhantomData,
        })
    }

    /// Validates the proposed name and decodes arguments into `T::Arguments`.
    ///
    /// Prefer [`ToolCall::validate`] when the application also needs to encode a
    /// result. This compatibility helper consumes the proof and returns only the
    /// validated arguments.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::UnexpectedName`] when the model proposed a different
    /// function, or [`ToolError::InvalidArguments`] when the arguments cannot be
    /// decoded into `T::Arguments`.
    pub fn arguments_for<T: ToolDefinition>(&self) -> Result<T::Arguments, ToolError> {
        Ok(self.validate::<T>()?.into_arguments())
    }

    fn ensure_name<T: ToolDefinition>(&self) -> Result<(), ToolError> {
        if self.name() != T::NAME {
            return Err(ToolError::UnexpectedName {
                expected: T::NAME,
                actual: self.name().to_owned(),
            });
        }
        Ok(())
    }
}

/// Why the provider stopped generating a completion.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FinishReason {
    /// The model reached a natural stopping point.
    Stop,
    /// The configured or provider token limit was reached.
    Length,
    /// The model proposed one or more tool calls.
    ToolCalls,
    /// The provider filtered generated content.
    ContentFilter,
    /// The provider returned a newer finish reason not yet modeled by this crate.
    #[serde(other)]
    Unknown,
}

/// Token accounting returned by the provider.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Usage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

impl Usage {
    /// Returns the number of input tokens reported by the provider.
    #[must_use]
    pub fn prompt_tokens(&self) -> u64 {
        self.prompt_tokens
    }

    /// Returns the number of generated tokens reported by the provider.
    #[must_use]
    pub fn completion_tokens(&self) -> u64 {
        self.completion_tokens
    }

    /// Returns the total token count reported by the provider.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens
    }
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::Deserialize;

    use super::*;

    struct GetWeather;

    impl ToolDefinition for GetWeather {
        type Arguments = WeatherArgs;
        type Output = WeatherReport;

        const NAME: &'static str = "get_weather";
        const DESCRIPTION: &'static str = "Get the weather for a city";
    }

    #[derive(Debug, Deserialize, JsonSchema, PartialEq)]
    struct WeatherArgs {
        city: String,
    }

    #[derive(Serialize)]
    struct WeatherReport {
        temperature_celsius: i16,
    }

    fn tool_call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: ToolCallId("call_1".into()),
            kind: ToolCallKind::Function,
            function: ToolCallFunction {
                name: name.into(),
                arguments: arguments.into(),
            },
        }
    }

    fn assistant_message(content: Option<&str>, tool_calls: Vec<ToolCall>) -> AssistantMessage {
        AssistantMessage {
            role: AssistantRole::Assistant,
            content: content.map(ToOwned::to_owned),
            tool_calls,
        }
    }

    #[test]
    fn serializes_required_function_tool_request() -> Result<(), Box<dyn std::error::Error>> {
        let request = ChatRequest::kimi_k3(vec![ChatMessage::user("What is the weather?")])?
            .with_tool(
                FunctionTool::for_tool::<GetWeather>()?,
                ToolChoice::Required,
            );

        let value = serde_json::to_value(request)?;
        assert_eq!(value["model"], "moonshotai/kimi-k3");
        assert_eq!(value["tool_choice"], "required");
        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(
            value["tools"][0]["function"]["parameters"]["type"],
            "object"
        );
        Ok(())
    }

    #[test]
    fn typestate_builder_preserves_the_existing_request_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let tool = FunctionTool::for_tool::<GetWeather>()?;
        let legacy = ChatRequest::kimi_k3(vec![ChatMessage::user("What is the weather?")])?
            .with_tool(tool.clone(), ToolChoice::Required);
        let typestate = ChatRequest::kimi_k3_builder()
            .tool(tool)
            .tool_choice(ToolChoice::Required)
            .message(ChatMessage::user("What is the weather?"))
            .build();

        assert_eq!(typestate, legacy);
        Ok(())
    }

    #[test]
    fn independent_builder_endomorphisms_commute() -> Result<(), Box<dyn std::error::Error>> {
        let temperature = Temperature::new(0.4)?;
        let max_tokens = MaxTokens::new(120)?;
        let temperature_then_tokens = ChatRequest::kimi_k3_builder()
            .temperature(temperature)
            .max_tokens(max_tokens)
            .message(ChatMessage::user("Explain composition"))
            .build();
        let tokens_then_temperature = ChatRequest::kimi_k3_builder()
            .max_tokens(max_tokens)
            .temperature(temperature)
            .message(ChatMessage::user("Explain composition"))
            .build();

        assert_eq!(temperature_then_tokens, tokens_then_temperature);
        Ok(())
    }

    #[test]
    fn classifies_every_assistant_output_sum_variant() {
        let call = tool_call("get_weather", r#"{"city":"Paris"}"#);
        let text = assistant_message(Some("Clear skies"), Vec::new());
        let tools = assistant_message(None, vec![call.clone()]);
        let both = assistant_message(Some("Checking"), vec![call]);
        let empty = assistant_message(None, Vec::new());

        assert_eq!(text.output(), AssistantOutput::Text("Clear skies"));
        assert!(matches!(
            tools.output(),
            AssistantOutput::ToolCalls(tool_calls) if tool_calls.len() == 1
        ));
        assert!(matches!(
            both.output(),
            AssistantOutput::TextAndToolCalls {
                text: "Checking",
                tool_calls
            } if tool_calls.len() == 1
        ));
        assert_eq!(empty.output(), AssistantOutput::Empty);
    }

    #[test]
    fn rejects_an_empty_message_list() {
        assert!(matches!(
            ChatRequest::kimi_k3(Vec::new()),
            Err(InvalidConfiguration::EmptyMessages)
        ));
    }

    #[test]
    fn rejects_an_empty_tool_definition_field() {
        struct InvalidTool;
        impl ToolDefinition for InvalidTool {
            type Arguments = WeatherArgs;
            type Output = WeatherReport;
            const NAME: &'static str = "";
            const DESCRIPTION: &'static str = "Description";
        }

        assert!(matches!(
            FunctionTool::for_tool::<InvalidTool>(),
            Err(ToolError::InvalidDefinition { field: "name" })
        ));
    }

    #[test]
    fn assistant_tool_call_round_trip_preserves_null_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let message = ChatMessage::assistant_tool_calls(vec![tool_call(
            "get_weather",
            r#"{"city":"Paris"}"#,
        )]);
        let value = serde_json::to_value(message)?;
        assert!(value["content"].is_null());
        assert_eq!(value["tool_calls"][0]["id"], "call_1");
        Ok(())
    }

    #[test]
    fn parses_typed_tool_arguments() -> Result<(), Box<dyn std::error::Error>> {
        let call = tool_call("get_weather", r#"{"city":"Paris"}"#);
        assert_eq!(
            call.arguments_for::<GetWeather>()?,
            WeatherArgs {
                city: "Paris".into()
            }
        );
        Ok(())
    }

    #[test]
    fn validated_tool_call_carries_arguments_and_output_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let call = tool_call("get_weather", r#"{"city":"Paris"}"#);
        let validated = call.validate::<GetWeather>()?;

        assert_eq!(validated.id(), "call_1");
        assert_eq!(validated.arguments().city, "Paris");
        assert_eq!(
            serde_json::to_value(validated.result(&WeatherReport {
                temperature_celsius: 18,
            })?)?,
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"temperature_celsius\":18}"
            })
        );
        Ok(())
    }

    #[test]
    fn rejects_arguments_for_a_different_typed_tool() {
        struct OtherTool;
        impl ToolDefinition for OtherTool {
            type Arguments = WeatherArgs;
            type Output = WeatherReport;
            const NAME: &'static str = "other_tool";
            const DESCRIPTION: &'static str = "A different tool";
        }

        let result = tool_call("get_weather", r#"{"city":"Paris"}"#).arguments_for::<OtherTool>();

        assert!(matches!(result, Err(ToolError::UnexpectedName { .. })));
    }

    #[test]
    fn rejects_malformed_tool_arguments_with_a_typed_error() {
        assert!(matches!(
            tool_call("get_weather", "not-json").arguments_for::<GetWeather>(),
            Err(ToolError::InvalidArguments { .. })
        ));
    }

    #[test]
    fn encodes_a_typed_tool_result_without_exposing_raw_json()
    -> Result<(), Box<dyn std::error::Error>> {
        let message = ChatMessage::tool_result::<GetWeather>(
            &tool_call("get_weather", r#"{"city":"Paris"}"#),
            &WeatherReport {
                temperature_celsius: 18,
            },
        )?;
        let encoded = serde_json::to_value(message)?;

        assert_eq!(encoded["tool_call_id"], "call_1");
        assert_eq!(encoded["content"], r#"{"temperature_celsius":18}"#);
        Ok(())
    }

    #[test]
    fn rejects_a_tool_result_for_a_different_contract() {
        struct OtherTool;
        impl ToolDefinition for OtherTool {
            type Arguments = WeatherArgs;
            type Output = WeatherReport;
            const NAME: &'static str = "other_tool";
            const DESCRIPTION: &'static str = "A different tool";
        }

        assert!(matches!(
            ChatMessage::tool_result::<OtherTool>(
                &tool_call("get_weather", r#"{"city":"Paris"}"#),
                &WeatherReport {
                    temperature_celsius: 18,
                },
            ),
            Err(ToolError::UnexpectedName { .. })
        ));
    }

    #[test]
    fn rejects_a_completion_without_choices() -> Result<(), Box<dyn std::error::Error>> {
        let completion: ChatCompletion = serde_json::from_value(serde_json::json!({
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "created": 1,
            "model": "moonshotai/kimi-k3",
            "choices": [],
            "usage": null
        }))?;

        assert!(matches!(
            completion.first_choice(),
            Err(Error::MissingChoice)
        ));
        Ok(())
    }

    #[test]
    fn rejects_an_unexpected_response_object() {
        let result = serde_json::from_value::<ChatCompletion>(serde_json::json!({
            "id": "chatcmpl-1",
            "object": "unexpected",
            "created": 1,
            "model": "moonshotai/kimi-k3",
            "choices": [],
            "usage": null
        }));

        assert!(result.is_err());
    }

    #[test]
    fn rejects_generation_values_outside_provider_contract() {
        assert!(matches!(
            Temperature::new(f32::NAN),
            Err(InvalidConfiguration::InvalidTemperature { .. })
        ));
        assert!(matches!(
            Temperature::new(2.1),
            Err(InvalidConfiguration::InvalidTemperature { .. })
        ));
        assert!(matches!(
            MaxTokens::new(0),
            Err(InvalidConfiguration::ZeroMaxTokens)
        ));
    }
}

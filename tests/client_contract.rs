use edge_completions::{
    AccountId, ApiBaseUrl, ApiToken, AssistantOutput, ChatCompletions, ChatMessage, ChatRequest,
    Client, FunctionTool, ToolChoice, ToolDefinition,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct LookupWeather;

impl ToolDefinition for LookupWeather {
    type Arguments = WeatherArguments;
    type Output = WeatherReport;

    const NAME: &'static str = "lookup_weather";
    const DESCRIPTION: &'static str = "Look up the weather for a validated city";
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
struct WeatherArguments {
    city: String,
}

#[derive(Serialize)]
struct WeatherReport {
    temperature_celsius: i16,
}

async fn complete_through_trait(
    completions: &dyn ChatCompletions,
    request: &ChatRequest,
) -> Result<edge_completions::ChatCompletion, edge_completions::Error> {
    completions.complete(request).await
}

#[tokio::test]
async fn downstream_caller_can_complete_and_decode_a_typed_tool_call() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/accounts/account-1/ai/v1/chat/completions"))
        .and(header("authorization", "Bearer test-token"))
        .and(body_json(json!({
            "model": "moonshotai/kimi-k3",
            "messages": [{"role": "user", "content": "Weather in Paris?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "lookup_weather",
                    "description": "Look up the weather for a validated city",
                    "parameters": {
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "title": "WeatherArguments",
                        "type": "object",
                        "properties": {"city": {"type": "string"}},
                        "required": ["city"]
                    }
                }
            }],
            "tool_choice": "required"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-contract-1",
            "object": "chat.completion",
            "created": 1,
            "model": "moonshotai/kimi-k3",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "lookup_weather",
                            "arguments": "{\"city\":\"Paris\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 8, "completion_tokens": 4, "total_tokens": 12}
        })))
        .mount(&server)
        .await;

    let client = Client::builder(AccountId::new("account-1")?, ApiToken::new("test-token")?)
        .base_url(ApiBaseUrl::new(format!("{}/", server.uri()))?)
        .build()?;
    let request = ChatRequest::kimi_k3_builder()
        .message(ChatMessage::user("Weather in Paris?"))
        .tool(FunctionTool::for_tool::<LookupWeather>()?)
        .tool_choice(ToolChoice::Required)
        .build();
    assert_eq!(
        serde_json::to_value(&request)?,
        json!({
            "model": "moonshotai/kimi-k3",
            "messages": [{"role": "user", "content": "Weather in Paris?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "lookup_weather",
                    "description": "Look up the weather for a validated city",
                    "parameters": {
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "title": "WeatherArguments",
                        "type": "object",
                        "properties": {"city": {"type": "string"}},
                        "required": ["city"]
                    }
                }
            }],
            "tool_choice": "required"
        })
    );

    let completion = complete_through_trait(&client, &request).await?;
    let calls = match completion.first_choice()?.message().output() {
        AssistantOutput::ToolCalls(calls)
        | AssistantOutput::TextAndToolCalls {
            tool_calls: calls, ..
        } => calls,
        AssistantOutput::Text(_) | AssistantOutput::Empty => {
            return Err("provider did not return the required tool-call alternative".into());
        }
    };
    let call = calls
        .first()
        .ok_or(edge_completions::Error::MissingToolCall)?;
    let validated_call = call.validate::<LookupWeather>()?;
    let result = validated_call.result(&WeatherReport {
        temperature_celsius: 18,
    })?;

    assert_eq!(completion.id(), "chatcmpl-contract-1");
    assert_eq!(validated_call.arguments().city, "Paris");
    assert_eq!(
        serde_json::to_value(result)?,
        json!({
            "role": "tool",
            "tool_call_id": "call-1",
            "content": "{\"temperature_celsius\":18}"
        })
    );
    Ok(())
}

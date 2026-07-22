use std::process::ExitCode;

use edge_completions::{
    AssistantOutput, ChatMessage, ChatRequest, Client, Error, FunctionTool, ToolChoice,
    ToolDefinition,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};

struct GetWeather;

impl ToolDefinition for GetWeather {
    type Arguments = WeatherArguments;
    type Output = WeatherReport;

    const NAME: &'static str = "get_weather";
    const DESCRIPTION: &'static str = "Get the example weather report for a city";
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WeatherArguments {
    city: CityName,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[schemars(transparent)]
struct CityName(String);

impl CityName {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CityName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(de::Error::custom(CityNameError));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("city name must not be empty")]
struct CityNameError;

#[derive(Debug, Serialize)]
struct WeatherReport {
    city: CityName,
    temperature: Celsius,
    condition: WeatherCondition,
    source: WeatherSource,
}

#[derive(Debug, Serialize)]
struct Celsius(i16);

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum WeatherCondition {
    Clear,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum WeatherSource {
    DeterministicExample,
}

fn get_weather(city: &CityName) -> WeatherReport {
    WeatherReport {
        city: city.clone(),
        temperature: Celsius(18),
        condition: WeatherCondition::Clear,
        source: WeatherSource::DeterministicExample,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Error> {
    let client = Client::from_env()?;
    let user_message = ChatMessage::user("What is the weather in San Francisco today?");
    let tool = FunctionTool::for_tool::<GetWeather>()?;

    let proposal = client
        .chat(
            &ChatRequest::kimi_k3_builder()
                .message(user_message.clone())
                .tool(tool.clone())
                .tool_choice(ToolChoice::Required)
                .build(),
        )
        .await?;
    let proposed_calls = proposal.first_choice()?.message().tool_calls().to_vec();
    let call = proposal.first_choice()?.message().first_tool_call()?;
    let validated_call = call.validate::<GetWeather>()?;

    println!("Tool requested: {}", call.name());
    println!(
        "Validated city: {}",
        validated_call.arguments().city.as_str()
    );

    // Tool execution is an explicit application decision, not an SDK side effect.
    let report = get_weather(&validated_call.arguments().city);
    let tool_result = validated_call.result(&report)?;

    let final_completion = client
        .chat(
            &ChatRequest::kimi_k3_builder()
                .message(user_message)
                .message(ChatMessage::assistant_tool_calls(proposed_calls))
                .message(tool_result)
                .tool(tool)
                .tool_choice(ToolChoice::Auto)
                .build(),
        )
        .await?;
    let answer = match final_completion.first_choice()?.message().output() {
        AssistantOutput::Text(text) | AssistantOutput::TextAndToolCalls { text, .. } => text,
        AssistantOutput::ToolCalls(_) | AssistantOutput::Empty => {
            return Err(Error::MissingContent);
        }
    };

    println!("Final answer: {answer}");
    Ok(())
}

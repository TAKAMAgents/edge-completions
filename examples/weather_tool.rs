use std::process::ExitCode;

use edge_completions::{
    ChatMessage, ChatRequest, Client, Error, FunctionTool, ToolChoice, ToolDefinition,
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
            &ChatRequest::kimi_k3(vec![user_message.clone()])?
                .with_tool(tool.clone(), ToolChoice::Required),
        )
        .await?;
    let proposed_calls = proposal.first_choice()?.message().tool_calls().to_vec();
    let call = proposal.first_choice()?.message().first_tool_call()?;
    let arguments = call.arguments_for::<GetWeather>()?;

    println!("Tool requested: {}", call.name());
    println!("Validated city: {}", arguments.city.as_str());

    // Tool execution is an explicit application decision, not an SDK side effect.
    let report = get_weather(&arguments.city);
    let tool_result = ChatMessage::tool_result::<GetWeather>(call, &report)?;

    let final_completion = client
        .chat(
            &ChatRequest::kimi_k3(vec![
                user_message,
                ChatMessage::assistant_tool_calls(proposed_calls),
                tool_result,
            ])?
            .with_tool(tool, ToolChoice::Auto),
        )
        .await?;
    let answer = final_completion
        .first_choice()?
        .message()
        .content()
        .ok_or(Error::MissingContent)?;

    println!("Final answer: {answer}");
    Ok(())
}

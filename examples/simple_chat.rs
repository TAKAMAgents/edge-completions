use std::process::ExitCode;

use edge_completions::{AssistantOutput, ChatMessage, ChatRequest, Client, Error};

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
    let request = ChatRequest::kimi_k3_builder()
        .message(ChatMessage::user(
            "Explain typed API boundaries in one concise sentence.",
        ))
        .build();

    let completion = client.chat(&request).await?;
    let answer = match completion.first_choice()?.message().output() {
        AssistantOutput::Text(text) | AssistantOutput::TextAndToolCalls { text, .. } => text,
        AssistantOutput::ToolCalls(_) | AssistantOutput::Empty => {
            return Err(Error::MissingContent);
        }
    };

    println!("Model: {}", completion.model());
    println!("Answer: {answer}");
    if let Some(usage) = completion.usage() {
        println!("Tokens: {}", usage.total_tokens());
    }

    Ok(())
}

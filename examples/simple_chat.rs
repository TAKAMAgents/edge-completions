use std::process::ExitCode;

use edge_completions::{ChatMessage, ChatRequest, Client, Error};

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
    let request = ChatRequest::kimi_k3(vec![ChatMessage::user(
        "Explain typed API boundaries in one concise sentence.",
    )])?;

    let completion = client.chat(&request).await?;
    let answer = completion
        .first_choice()?
        .message()
        .content()
        .ok_or(Error::MissingContent)?;

    println!("Model: {}", completion.model());
    println!("Answer: {answer}");
    if let Some(usage) = completion.usage() {
        println!("Tokens: {}", usage.total_tokens());
    }

    Ok(())
}

use std::{
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, error::ErrorKind};
use edge_completions::{
    ApiBaseUrl, AssistantOutput, ChatCompletions, ChatMessage, ChatRequest, Client, Error,
    GatewayId, InvalidConfiguration, MaxTokens, ModelId, Temperature,
};

const MAX_PROMPT_BYTES: usize = 64 * 1024;
const USAGE_ERROR_EXIT_CODE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "edge-completions",
    version,
    about = "Typed Cloudflare chat completions from the command line",
    long_about = "Send typed, non-streaming chat-completion requests through Cloudflare.\n\
                  Credentials are read from CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN; \
                  KIMI3_ON_CLOUDFLARE_API_KEY remains a token fallback. Provider envelopes and \
                  credentials are never printed."
)]
struct CommandLine {
    /// Override the Cloudflare API base URL.
    ///
    /// HTTPS is required except for loopback addresses used by local contract
    /// tests. The bearer token is sent to the selected endpoint.
    #[arg(long, global = true, value_name = "URL")]
    base_url: Option<ApiBaseUrl>,

    /// Route the request through a Cloudflare AI Gateway.
    #[arg(long, global = true, value_name = "GATEWAY_ID")]
    gateway_id: Option<GatewayId>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate local credentials and transport configuration without an API call.
    Check,

    /// Send one chat request and print only the assistant text.
    Chat(ChatArguments),
}

#[derive(Debug, Args)]
struct ChatArguments {
    /// User prompt. When omitted, the prompt is read from standard input.
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    /// Provider model identifier.
    #[arg(
        short,
        long,
        value_name = "MODEL",
        default_value = "moonshotai/kimi-k3"
    )]
    model: ModelId,

    /// Optional system instruction sent before the user prompt.
    #[arg(short, long, value_name = "TEXT")]
    system: Option<String>,

    /// Sampling temperature in the inclusive range 0 through 2.
    #[arg(long, value_name = "NUMBER")]
    temperature: Option<f32>,

    /// Non-zero maximum completion-token count.
    #[arg(long, value_name = "TOKENS")]
    max_tokens: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error(transparent)]
    Sdk(#[from] Error),

    #[error(transparent)]
    InvalidConfiguration(#[from] InvalidConfiguration),

    #[error("provide a non-empty PROMPT argument or pipe a prompt through standard input")]
    MissingPrompt,

    #[error("prompt exceeds the {limit_bytes}-byte command limit")]
    PromptTooLarge { limit_bytes: usize },

    #[error("failed to read the prompt from standard input: {source}")]
    ReadPrompt {
        #[source]
        source: io::Error,
    },

    #[error("failed to initialize the async runtime: {source}")]
    Runtime {
        #[source]
        source: io::Error,
    },

    #[error("failed to write command output: {source}")]
    WriteOutput {
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Prompt(String);

impl Prompt {
    fn new(value: String) -> Result<Self, CommandError> {
        if value.trim().is_empty() {
            return Err(CommandError::MissingPrompt);
        }
        if value.len() > MAX_PROMPT_BYTES {
            return Err(CommandError::PromptTooLarge {
                limit_bytes: MAX_PROMPT_BYTES,
            });
        }
        Ok(Self(value))
    }

    fn from_argument_or_reader(
        argument: Option<String>,
        input: impl Read,
        input_is_terminal: bool,
    ) -> Result<Self, CommandError> {
        if let Some(value) = argument {
            return Self::new(value);
        }
        if input_is_terminal {
            return Err(CommandError::MissingPrompt);
        }

        let mut value = String::new();
        input
            .take((MAX_PROMPT_BYTES + 1) as u64)
            .read_to_string(&mut value)
            .map_err(|source| CommandError::ReadPrompt { source })?;
        Self::new(value)
    }

    fn into_inner(self) -> String {
        self.0
    }
}

fn main() -> ExitCode {
    match CommandLine::try_parse() {
        Ok(command_line) => exit_for(run(command_line)),
        Err(error) => match emit_parse_result(error) {
            Ok(exit_code) => exit_code,
            Err(error) => report_error(&error),
        },
    }
}

fn run(command_line: CommandLine) -> Result<(), CommandError> {
    let CommandLine {
        base_url,
        gateway_id,
        command,
    } = command_line;

    match command {
        Command::Check => {
            configured_client(base_url, gateway_id)?;
            write_line(io::stdout().lock(), "configuration is valid")
        }
        Command::Chat(mut arguments) => {
            let input_is_terminal = io::stdin().is_terminal();
            let prompt = Prompt::from_argument_or_reader(
                arguments.prompt.take(),
                io::stdin().lock(),
                input_is_terminal,
            )?;
            let request = arguments.into_request(prompt)?;
            let client = configured_client(base_url, gateway_id)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|source| CommandError::Runtime { source })?;
            let answer = runtime.block_on(complete_text(&client, &request))?;
            write_line(io::stdout().lock(), &answer)
        }
    }
}

fn configured_client(
    base_url: Option<ApiBaseUrl>,
    gateway_id: Option<GatewayId>,
) -> Result<Client, CommandError> {
    let mut builder = Client::builder_from_env()?;
    if let Some(base_url) = base_url {
        builder = builder.base_url(base_url);
    }
    if let Some(gateway_id) = gateway_id {
        builder = builder.gateway_id(gateway_id);
    }
    Ok(builder.build()?)
}

impl ChatArguments {
    fn into_request(self, prompt: Prompt) -> Result<ChatRequest, CommandError> {
        let user_message = ChatMessage::user(prompt.into_inner());
        let mut builder = match self.system {
            Some(system) => ChatRequest::builder(self.model)
                .message(ChatMessage::system(system))
                .message(user_message),
            None => ChatRequest::builder(self.model).message(user_message),
        };
        if let Some(value) = self.temperature {
            builder = builder.temperature(Temperature::new(value)?);
        }
        if let Some(value) = self.max_tokens {
            builder = builder.max_tokens(MaxTokens::new(value)?);
        }
        Ok(builder.build())
    }
}

async fn complete_text(
    completions: &dyn ChatCompletions,
    request: &ChatRequest,
) -> Result<String, CommandError> {
    let completion = completions.complete(request).await?;
    match completion.first_choice()?.message().output() {
        AssistantOutput::Text(text) | AssistantOutput::TextAndToolCalls { text, .. } => {
            Ok(text.to_owned())
        }
        AssistantOutput::ToolCalls(_) | AssistantOutput::Empty => Err(Error::MissingContent.into()),
    }
}

fn emit_parse_result(error: clap::Error) -> Result<ExitCode, CommandError> {
    let exit_code = match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::SUCCESS,
        _ => ExitCode::from(USAGE_ERROR_EXIT_CODE),
    };
    let rendered = error.to_string();
    if error.use_stderr() {
        write_text(io::stderr().lock(), &rendered)?;
    } else {
        write_text(io::stdout().lock(), &rendered)?;
    }
    Ok(exit_code)
}

fn exit_for(result: Result<(), CommandError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => report_error(&error),
    }
}

fn report_error(error: &CommandError) -> ExitCode {
    let message = format!("edge-completions: {error}\n");
    let _write_result = write_text(io::stderr().lock(), &message);
    ExitCode::FAILURE
}

fn write_line(output: impl Write, value: &str) -> Result<(), CommandError> {
    write_text(output, &format!("{value}\n"))
}

fn write_text(mut output: impl Write, value: &str) -> Result<(), CommandError> {
    output
        .write_all(value.as_bytes())
        .map_err(|source| CommandError::WriteOutput { source })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::json;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parses_typed_chat_options() -> TestResult {
        let command_line = CommandLine::try_parse_from([
            "edge-completions",
            "chat",
            "Explain typed boundaries",
            "--temperature",
            "0.4",
            "--max-tokens",
            "120",
        ])?;

        let Command::Chat(arguments) = command_line.command else {
            return Err("chat subcommand was not parsed".into());
        };
        assert_eq!(arguments.model, ModelId::kimi_k3());
        assert_eq!(arguments.temperature, Some(0.4));
        assert_eq!(arguments.max_tokens, Some(120));
        Ok(())
    }

    #[test]
    fn help_is_a_successful_parse_result() -> TestResult {
        let error = match CommandLine::try_parse_from(["edge-completions", "--help"]) {
            Ok(_) => return Err("help unexpectedly parsed as an executable command".into()),
            Err(error) => error,
        };

        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        Ok(())
    }

    #[test]
    fn reads_a_piped_prompt_and_builds_a_typed_request() -> TestResult {
        let prompt =
            Prompt::from_argument_or_reader(None, Cursor::new("Explain typed boundaries"), false)?;
        let arguments = ChatArguments {
            prompt: None,
            model: ModelId::kimi_k3(),
            system: Some("Be concise".to_owned()),
            temperature: Some(0.5),
            max_tokens: Some(80),
        };

        let request = arguments.into_request(prompt)?;
        assert_eq!(
            serde_json::to_value(request)?,
            json!({
                "model": "moonshotai/kimi-k3",
                "messages": [
                    {"role": "system", "content": "Be concise"},
                    {"role": "user", "content": "Explain typed boundaries"}
                ],
                "temperature": 0.5,
                "max_tokens": 80
            })
        );
        Ok(())
    }

    #[test]
    fn rejects_an_oversized_prompt_before_an_api_call() {
        let result = Prompt::new("x".repeat(MAX_PROMPT_BYTES + 1));
        assert!(matches!(result, Err(CommandError::PromptTooLarge { .. })));
    }

    #[test]
    fn requires_a_prompt_in_an_interactive_terminal() {
        let result = Prompt::from_argument_or_reader(None, Cursor::new(Vec::<u8>::new()), true);
        assert!(matches!(result, Err(CommandError::MissingPrompt)));
    }
}

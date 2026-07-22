#![cfg(feature = "cli")]

use std::process::Command;

use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn check_reports_a_typed_missing_configuration_error() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_edge-completions"))
        .arg("check")
        .env_remove("CLOUDFLARE_ACCOUNT_ID")
        .env_remove("CLOUDFLARE_API_TOKEN")
        .env_remove("KIMI3_ON_CLOUDFLARE_API_KEY")
        .output()?;

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr)?,
        "edge-completions: required environment variable CLOUDFLARE_ACCOUNT_ID is missing or is not valid Unicode\n"
    );
    Ok(())
}

#[test]
fn chat_validates_local_invariants_before_credentials() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_edge-completions"))
        .args(["chat", "Say hello", "--temperature", "3"])
        .env_remove("CLOUDFLARE_ACCOUNT_ID")
        .env_remove("CLOUDFLARE_API_TOKEN")
        .env_remove("KIMI3_ON_CLOUDFLARE_API_KEY")
        .output()?;

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr)?,
        "edge-completions: temperature must be finite and between 0 and 2 inclusive, got 3\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_prints_only_typed_assistant_content() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/accounts/account-1/ai/v1/chat/completions"))
        .and(header("authorization", "Bearer test-token"))
        .and(body_json(json!({
            "model": "moonshotai/kimi-k3",
            "messages": [{"role": "user", "content": "Say hello"}],
            "temperature": 0.2,
            "max_tokens": 16
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-command-contract",
            "object": "chat.completion",
            "created": 1,
            "model": "moonshotai/kimi-k3",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello from the typed CLI."},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 2, "completion_tokens": 5, "total_tokens": 7}
        })))
        .mount(&server)
        .await;

    let base_url = format!("{}/", server.uri());
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_edge-completions"))
            .args([
                "--base-url",
                &base_url,
                "chat",
                "Say hello",
                "--temperature",
                "0.2",
                "--max-tokens",
                "16",
            ])
            .env("CLOUDFLARE_ACCOUNT_ID", "account-1")
            .env("CLOUDFLARE_API_TOKEN", "test-token")
            .env_remove("KIMI3_ON_CLOUDFLARE_API_KEY")
            .output()
    })
    .await??;

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "Hello from the typed CLI.\n"
    );
    assert!(output.stderr.is_empty());
    Ok(())
}

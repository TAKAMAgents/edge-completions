use std::{io, time::Duration};

use edge_completions::{
    AccountId, ApiBaseUrl, ApiToken, ChatCompletion, ChatCompletions, ChatMessage, ChatRequest,
    Client, DynChatCompletions, Error, RequestTimeout,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct UnavailableCompletions;

impl ChatCompletions for UnavailableCompletions {
    async fn complete<'a>(&'a self, _request: &'a ChatRequest) -> Result<ChatCompletion, Error> {
        Err(Error::MissingChoice)
    }
}

fn assert_send<T: Send>(_: &T) {}

fn successful_completion(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "object": "chat.completion",
        "created": 1,
        "model": "moonshotai/kimi-k3",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "typed result"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 2,
            "completion_tokens": 2,
            "total_tokens": 4
        }
    })
}

fn request(prompt: &str) -> ChatRequest {
    ChatRequest::kimi_k3_builder()
        .message(ChatMessage::user(prompt))
        .build()
}

fn client_for(server: &MockServer) -> Result<Client, Error> {
    Client::builder(AccountId::new("account-1")?, ApiToken::new("test-token")?)
        .base_url(ApiBaseUrl::new(format!("{}/", server.uri()))?)
        .build()
}

async fn wait_for_request_count(server: &MockServer, expected: usize) -> Result<(), io::Error> {
    for _ in 0..100 {
        let received = server
            .received_requests()
            .await
            .map_or(0, |requests| requests.len());
        if received >= expected {
            return Ok(());
        }
        tokio::task::yield_now().await;
    }

    Err(io::Error::other(
        "mock server did not receive the expected request",
    ))
}

#[tokio::test]
async fn native_future_is_send_and_dynamic_erasure_is_explicit() -> TestResult {
    let completions = UnavailableCompletions;
    let request = request("hello");

    let native_future = completions.complete(&request);
    assert_send(&native_future);
    drop(native_future);

    let erased: &dyn DynChatCompletions = &completions;
    assert!(matches!(
        erased.complete_boxed(&request).await,
        Err(Error::MissingChoice)
    ));
    Ok(())
}

#[tokio::test]
async fn cloned_client_supports_concurrent_native_calls() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(successful_completion("chatcmpl-concurrent")),
        )
        .expect(2)
        .mount(&server)
        .await;
    let client = client_for(&server)?;
    let first_request = request("first");
    let second_request = request("second");

    let (first, second) = tokio::join!(
        client.complete(&first_request),
        client.complete(&second_request)
    );

    assert_eq!(first?.id(), "chatcmpl-concurrent");
    assert_eq!(second?.id(), "chatcmpl-concurrent");
    Ok(())
}

#[tokio::test]
async fn configured_deadline_returns_typed_timeout() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(30))
                .set_body_json(successful_completion("chatcmpl-too-late")),
        )
        .mount(&server)
        .await;
    let client = Client::builder(AccountId::new("account-1")?, ApiToken::new("test-token")?)
        .base_url(ApiBaseUrl::new(format!("{}/", server.uri()))?)
        .timeout(RequestTimeout::new(Duration::from_secs(1))?)
        .build()?;
    let request = request("timeout");

    tokio::time::pause();
    let task = tokio::spawn(async move { client.complete(&request).await });
    wait_for_request_count(&server, 1).await?;
    tokio::time::advance(Duration::from_secs(2)).await;
    let error = match task.await? {
        Err(error) => error,
        Ok(_) => return Err("request unexpectedly completed before its deadline".into()),
    };

    match error {
        Error::Timeout { duration, source } => {
            assert_eq!(duration, Duration::from_secs(1));
            assert!(source.is_timeout());
        }
        other => return Err(format!("expected a typed timeout, got {other}").into()),
    }
    Ok(())
}

#[tokio::test]
async fn caller_timeout_cancels_without_poisoning_the_client() -> TestResult {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (first_request_received, first_request_observed) = oneshot::channel();
    let (first_connection_closed, first_connection_close_observed) = oneshot::channel();
    let (second_request_received, second_request_observed) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut slow_socket, _) = listener.accept().await?;
        let mut request_buffer = [0_u8; 2_048];
        let _bytes_read = slow_socket.read(&mut request_buffer).await?;
        let _receiver_still_waiting = first_request_received.send(());
        while slow_socket.read(&mut request_buffer).await? != 0 {}
        let _receiver_still_waiting = first_connection_closed.send(());

        let (mut fast_socket, _) = listener.accept().await?;
        let _bytes_read = fast_socket.read(&mut request_buffer).await?;
        let _receiver_still_waiting = second_request_received.send(());
        let body = successful_completion("chatcmpl-fast").to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        fast_socket.write_all(response.as_bytes()).await?;
        fast_socket.shutdown().await
    });
    let client = Client::builder(AccountId::new("account-1")?, ApiToken::new("test-token")?)
        .base_url(ApiBaseUrl::new(format!("http://{address}/"))?)
        .build()?;
    let slow_request = request("slow");

    let cancellable_client = client.clone();
    let task = tokio::spawn(async move {
        tokio::time::timeout(
            Duration::from_millis(250),
            cancellable_client.complete(&slow_request),
        )
        .await
    });
    first_request_observed.await?;
    assert!(task.await?.is_err());
    tokio::time::timeout(Duration::from_secs(1), first_connection_close_observed)
        .await
        .map_err(|_| io::Error::other("cancelled request did not close its connection"))??;

    let fast_request = request("fast");
    let task = tokio::spawn(async move { client.complete(&fast_request).await });
    second_request_observed.await?;
    let completion = task.await??;
    assert_eq!(completion.id(), "chatcmpl-fast");
    server.await??;
    Ok(())
}

#[tokio::test]
async fn dropped_response_connection_is_a_transport_error() -> TestResult {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        let mut request_buffer = [0_u8; 2_048];
        let _bytes_read = socket.read(&mut request_buffer).await?;
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                  Content-Length: 128\r\nConnection: close\r\n\r\n{\"id\":\"partial",
            )
            .await?;
        socket.shutdown().await
    });
    let client = Client::builder(AccountId::new("account-1")?, ApiToken::new("test-token")?)
        .base_url(ApiBaseUrl::new(format!("http://{address}/"))?)
        .build()?;

    let error = match client.complete(&request("disconnect")).await {
        Err(error) => error,
        Ok(_) => return Err("partial response unexpectedly decoded".into()),
    };
    server.await??;

    assert!(matches!(error, Error::Transport { .. }));
    Ok(())
}

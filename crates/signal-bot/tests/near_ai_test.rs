//! Integration tests for NEAR AI client.

mod common;

use common::{mock_near_ai_server, test_near_ai_client};
use near_ai_client::Message;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn test_near_ai_chat_integration() {
    let mock_server = mock_near_ai_server().await;
    let client = test_near_ai_client(&mock_server);

    let response_body = serde_json::json!({
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Integration test response"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 15,
            "total_tokens": 25
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hello from integration test")];
    let result = client.chat(messages, None, None).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Integration test response");
}

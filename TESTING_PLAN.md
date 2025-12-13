# Testing Plan: Signal Bot TEE

## Overview

This document outlines the comprehensive testing strategy for the Signal Bot TEE project, addressing issues identified in the code review and ensuring production readiness.

**Testing Priorities:**
1. Fix critical security issues before testing
2. Unit tests for core logic
3. Integration tests with mocked services
4. End-to-end tests with real services (optional, local)

---

## Pre-Testing: Critical Fixes Required

Before writing tests, address these critical issues:

### Fix 1: API Key Security (Critical)
```rust
// near-ai-client/src/client.rs
use secrecy::{ExposeSecret, SecretString};

pub struct NearAiClient {
    client: Client,
    base_url: String,
    api_key: SecretString,  // Never logged or serialized
    model: String,
}
```

### Fix 2: Empty Response Handling (Critical)
```rust
// near-ai-client/src/client.rs - chat() method
.and_then(|r| {
    r.choices
        .into_iter()
        .next()
        .ok_or_else(|| NearAiError::Api {
            status: 200,
            message: "No choices in response".into(),
        })
        .map(|c| c.message.content)
})
```

### Fix 3: Add Health Check to ConversationStore
```rust
// conversation-store/src/store.rs
pub async fn health_check(&self) -> Result<(), ConversationError> {
    let mut conn = self.conn.clone();
    let _: String = redis::cmd("PING").query_async(&mut conn).await?;
    Ok(())
}
```

---

## Test Infrastructure Setup

### Dependencies to Add

**Workspace Cargo.toml:**
```toml
[workspace.dependencies]
# Testing
tokio-test = "0.4"
mockall = "0.12"
wiremock = "0.5"
testcontainers = "0.15"
assert_matches = "1.5"
fake = { version = "2.9", features = ["derive"] }
proptest = "1.4"
```

### Test Directory Structure
```
tests/
├── common/
│   ├── mod.rs              # Shared test utilities
│   ├── fixtures.rs         # Test data generators
│   └── mocks.rs            # Mock implementations
├── unit/
│   ├── near_ai_test.rs
│   ├── conversation_test.rs
│   ├── dstack_test.rs
│   ├── signal_test.rs
│   └── commands_test.rs
├── integration/
│   ├── chat_flow_test.rs
│   ├── verify_flow_test.rs
│   └── redis_test.rs
└── e2e/
    └── full_bot_test.rs
```

---

## Phase 1: Unit Tests

### 1.1 near-ai-client Tests

**File: `crates/near-ai-client/src/client.rs` (add tests module)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};

    async fn setup_mock_server() -> MockServer {
        MockServer::start().await
    }

    #[tokio::test]
    async fn test_chat_success() {
        let server = setup_mock_server().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "test-id",
                "object": "chat.completion",
                "created": 1234567890,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "Hello!"},
                    "finish_reason": "stop"
                }],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            })))
            .mount(&server)
            .await;

        let client = NearAiClient::new(
            "test-key",
            &server.uri(),
            "test-model",
            Duration::from_secs(10),
        ).unwrap();

        let messages = vec![Message::user("Hi")];
        let result = client.chat(messages, None, None).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello!");
    }

    #[tokio::test]
    async fn test_chat_empty_choices_returns_error() {
        let server = setup_mock_server().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "test-id",
                "object": "chat.completion",
                "created": 1234567890,
                "model": "test-model",
                "choices": [],  // Empty choices
                "usage": null
            })))
            .mount(&server)
            .await;

        let client = NearAiClient::new("test-key", &server.uri(), "test-model", Duration::from_secs(10)).unwrap();
        let result = client.chat(vec![Message::user("Hi")], None, None).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_chat_rate_limit() {
        let server = setup_mock_server().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let client = NearAiClient::new("test-key", &server.uri(), "test-model", Duration::from_secs(10)).unwrap();
        let result = client.chat(vec![Message::user("Hi")], None, None).await;

        assert!(matches!(result, Err(NearAiError::RateLimit)));
    }

    #[tokio::test]
    async fn test_chat_unauthorized() {
        let server = setup_mock_server().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = NearAiClient::new("bad-key", &server.uri(), "test-model", Duration::from_secs(10)).unwrap();
        let result = client.chat(vec![Message::user("Hi")], None, None).await;

        assert!(matches!(result, Err(NearAiError::Unauthorized)));
    }

    #[tokio::test]
    async fn test_health_check_success() {
        let server = setup_mock_server().await;

        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"id": "model1", "object": "model", "created": 0, "owned_by": "test"}]
            })))
            .mount(&server)
            .await;

        let client = NearAiClient::new("test-key", &server.uri(), "test-model", Duration::from_secs(10)).unwrap();
        assert!(client.health_check().await);
    }

    #[tokio::test]
    async fn test_health_check_failure() {
        let server = setup_mock_server().await;

        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = NearAiClient::new("test-key", &server.uri(), "test-model", Duration::from_secs(10)).unwrap();
        assert!(!client.health_check().await);
    }
}
```

### 1.2 conversation-store Tests

**File: `crates/conversation-store/src/store.rs` (add tests module)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use testcontainers::{clients::Cli, images::redis::Redis};

    async fn setup_redis() -> (Cli, String) {
        let docker = Cli::default();
        let container = docker.run(Redis::default());
        let port = container.get_host_port_ipv4(6379);
        let url = format!("redis://127.0.0.1:{}", port);
        (docker, url)
    }

    #[tokio::test]
    async fn test_add_and_get_message() {
        let (_docker, url) = setup_redis().await;
        let store = ConversationStore::new(&url, 50, Duration::from_secs(3600)).await.unwrap();

        let conv = store.add_message("user123", "user", "Hello", Some("System prompt")).await.unwrap();

        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, "user");
        assert_eq!(conv.messages[0].content, "Hello");
        assert_eq!(conv.system_prompt, Some("System prompt".to_string()));
    }

    #[tokio::test]
    async fn test_conversation_trimming() {
        let (_docker, url) = setup_redis().await;
        let store = ConversationStore::new(&url, 3, Duration::from_secs(3600)).await.unwrap();

        for i in 0..5 {
            store.add_message("user123", "user", &format!("Message {}", i), None).await.unwrap();
        }

        let conv = store.get("user123").await.unwrap().unwrap();
        assert_eq!(conv.messages.len(), 3);  // Trimmed to max
        assert_eq!(conv.messages[0].content, "Message 2");  // Oldest kept
        assert_eq!(conv.messages[2].content, "Message 4");  // Newest
    }

    #[tokio::test]
    async fn test_clear_conversation() {
        let (_docker, url) = setup_redis().await;
        let store = ConversationStore::new(&url, 50, Duration::from_secs(3600)).await.unwrap();

        store.add_message("user123", "user", "Hello", None).await.unwrap();
        let cleared = store.clear("user123").await.unwrap();

        assert!(cleared);
        assert!(store.get("user123").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_clear_nonexistent_conversation() {
        let (_docker, url) = setup_redis().await;
        let store = ConversationStore::new(&url, 50, Duration::from_secs(3600)).await.unwrap();

        let cleared = store.clear("nonexistent").await.unwrap();
        assert!(!cleared);
    }

    #[tokio::test]
    async fn test_to_openai_messages() {
        let (_docker, url) = setup_redis().await;
        let store = ConversationStore::new(&url, 50, Duration::from_secs(3600)).await.unwrap();

        store.add_message("user123", "user", "Hello", Some("Be helpful")).await.unwrap();
        store.add_message("user123", "assistant", "Hi there!", None).await.unwrap();

        let messages = store.to_openai_messages("user123", None).await.unwrap();

        assert_eq!(messages.len(), 3);  // system + user + assistant
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "Be helpful");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
    }

    #[tokio::test]
    async fn test_message_count() {
        let (_docker, url) = setup_redis().await;
        let store = ConversationStore::new(&url, 50, Duration::from_secs(3600)).await.unwrap();

        assert_eq!(store.message_count("user123").await.unwrap(), 0);

        store.add_message("user123", "user", "Hello", None).await.unwrap();
        assert_eq!(store.message_count("user123").await.unwrap(), 1);
    }
}
```

### 1.3 signal-client Tests

**File: `crates/signal-client/src/types.rs` (add tests)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_incoming_message() -> IncomingMessage {
        IncomingMessage {
            envelope: Envelope {
                source: "+1234567890".to_string(),
                source_number: Some("+1234567890".to_string()),
                source_name: Some("Test User".to_string()),
                timestamp: 1234567890,
                data_message: Some(DataMessage {
                    message: Some("Hello bot".to_string()),
                    timestamp: 1234567890,
                    group_info: None,
                }),
            },
            account: "+0987654321".to_string(),
        }
    }

    #[test]
    fn test_bot_message_from_incoming() {
        let incoming = sample_incoming_message();
        let bot_msg = BotMessage::from_incoming(&incoming).unwrap();

        assert_eq!(bot_msg.source, "+1234567890");
        assert_eq!(bot_msg.text, "Hello bot");
        assert!(!bot_msg.is_group);
        assert!(bot_msg.group_id.is_none());
    }

    #[test]
    fn test_bot_message_from_group() {
        let mut incoming = sample_incoming_message();
        incoming.envelope.data_message.as_mut().unwrap().group_info = Some(GroupInfo {
            group_id: "group123".to_string(),
        });

        let bot_msg = BotMessage::from_incoming(&incoming).unwrap();

        assert!(bot_msg.is_group);
        assert_eq!(bot_msg.group_id, Some("group123".to_string()));
    }

    #[test]
    fn test_bot_message_reply_target_direct() {
        let incoming = sample_incoming_message();
        let bot_msg = BotMessage::from_incoming(&incoming).unwrap();

        assert_eq!(bot_msg.reply_target(), "+1234567890");
    }

    #[test]
    fn test_bot_message_reply_target_group() {
        let mut incoming = sample_incoming_message();
        incoming.envelope.data_message.as_mut().unwrap().group_info = Some(GroupInfo {
            group_id: "group123".to_string(),
        });

        let bot_msg = BotMessage::from_incoming(&incoming).unwrap();
        assert_eq!(bot_msg.reply_target(), "group123");
    }

    #[test]
    fn test_bot_message_no_text_returns_none() {
        let mut incoming = sample_incoming_message();
        incoming.envelope.data_message.as_mut().unwrap().message = None;

        assert!(BotMessage::from_incoming(&incoming).is_none());
    }

    #[test]
    fn test_bot_message_no_data_message_returns_none() {
        let mut incoming = sample_incoming_message();
        incoming.envelope.data_message = None;

        assert!(BotMessage::from_incoming(&incoming).is_none());
    }
}
```

### 1.4 Command Handler Tests

**File: `crates/signal-bot/src/commands/help.rs` (add tests)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message() -> BotMessage {
        BotMessage {
            source: "+1234567890".to_string(),
            text: "!help".to_string(),
            timestamp: 1234567890,
            is_group: false,
            group_id: None,
        }
    }

    #[tokio::test]
    async fn test_help_command_matches() {
        let handler = HelpHandler::new();
        let msg = sample_message();

        assert!(handler.matches(&msg));
    }

    #[tokio::test]
    async fn test_help_command_not_matches_other() {
        let handler = HelpHandler::new();
        let mut msg = sample_message();
        msg.text = "!verify".to_string();

        assert!(!handler.matches(&msg));
    }

    #[tokio::test]
    async fn test_help_returns_help_text() {
        let handler = HelpHandler::new();
        let msg = sample_message();

        let result = handler.execute(&msg).await.unwrap();

        assert!(result.contains("!verify"));
        assert!(result.contains("!clear"));
        assert!(result.contains("!models"));
        assert!(result.contains("!help"));
    }
}
```

---

## Phase 2: Integration Tests

### 2.1 Chat Flow Integration Test

**File: `tests/integration/chat_flow_test.rs`**

```rust
//! Integration test for complete chat flow

use near_ai_client::NearAiClient;
use conversation_store::ConversationStore;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{clients::Cli, images::redis::Redis};
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

async fn setup() -> (MockServer, Arc<NearAiClient>, Arc<ConversationStore>) {
    // Start mock NEAR AI server
    let near_server = MockServer::start().await;

    // Start Redis
    let docker = Cli::default();
    let redis_container = docker.run(Redis::default());
    let redis_port = redis_container.get_host_port_ipv4(6379);
    let redis_url = format!("redis://127.0.0.1:{}", redis_port);

    let near_ai = Arc::new(NearAiClient::new(
        "test-key",
        &near_server.uri(),
        "test-model",
        Duration::from_secs(10),
    ).unwrap());

    let conversations = Arc::new(ConversationStore::new(
        &redis_url,
        50,
        Duration::from_secs(3600),
    ).await.unwrap());

    (near_server, near_ai, conversations)
}

#[tokio::test]
async fn test_full_chat_flow() {
    let (server, near_ai, conversations) = setup().await;

    // Mock NEAR AI response
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "test",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "I can help with that!"},
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    // Simulate user message
    let user_id = "+1234567890";
    let system_prompt = "You are helpful";

    // Add user message
    conversations.add_message(user_id, "user", "How do I test Rust?", Some(system_prompt)).await.unwrap();

    // Get conversation and send to NEAR AI
    let messages = conversations.to_openai_messages(user_id, Some(system_prompt)).await.unwrap();
    let near_messages: Vec<_> = messages.into_iter().map(|m| {
        near_ai_client::Message {
            role: match m.role.as_str() {
                "system" => near_ai_client::Role::System,
                "assistant" => near_ai_client::Role::Assistant,
                _ => near_ai_client::Role::User,
            },
            content: m.content,
        }
    }).collect();

    let response = near_ai.chat(near_messages, Some(0.7), None).await.unwrap();

    // Store response
    conversations.add_message(user_id, "assistant", &response, None).await.unwrap();

    // Verify conversation state
    let conv = conversations.get(user_id).await.unwrap().unwrap();
    assert_eq!(conv.messages.len(), 2);
    assert_eq!(conv.messages[1].content, "I can help with that!");
}

#[tokio::test]
async fn test_conversation_context_maintained() {
    let (server, near_ai, conversations) = setup().await;

    // First exchange
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "test",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "My name is Signal Bot."},
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let user_id = "+1234567890";

    // First message
    conversations.add_message(user_id, "user", "What is your name?", Some("You are Signal Bot")).await.unwrap();
    let _ = near_ai.chat(vec![near_ai_client::Message::user("What is your name?")], None, None).await.unwrap();
    conversations.add_message(user_id, "assistant", "My name is Signal Bot.", None).await.unwrap();

    // Second message - should include context
    conversations.add_message(user_id, "user", "Can you repeat that?", None).await.unwrap();
    let messages = conversations.to_openai_messages(user_id, None).await.unwrap();

    // Verify context includes previous messages
    assert_eq!(messages.len(), 4);  // system + user1 + assistant + user2
    assert!(messages.iter().any(|m| m.content.contains("What is your name?")));
    assert!(messages.iter().any(|m| m.content.contains("My name is Signal Bot")));
}
```

---

## Phase 3: Property-Based Tests

### 3.1 Conversation Trimming Properties

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn conversation_never_exceeds_max_messages(
        messages in prop::collection::vec(any::<String>(), 0..100),
        max_messages in 1usize..50
    ) {
        let mut conv = Conversation::new("test", None);
        for msg in messages {
            conv.add_message("user", &msg);
            conv.trim(max_messages);
            assert!(conv.messages.len() <= max_messages);
        }
    }

    #[test]
    fn trimming_keeps_most_recent_messages(
        messages in prop::collection::vec("[a-z]{1,10}".prop_map(|s| s), 10..20)
    ) {
        let mut conv = Conversation::new("test", None);
        for msg in &messages {
            conv.add_message("user", msg);
        }
        conv.trim(5);

        // Last 5 messages should be preserved
        let last_5: Vec<_> = messages.iter().rev().take(5).rev().collect();
        for (i, expected) in last_5.iter().enumerate() {
            assert_eq!(&conv.messages[i].content, *expected);
        }
    }
}
```

---

## Phase 4: Mock Trait Implementations

### 4.1 Create Mock Traits for Testing

**File: `tests/common/mocks.rs`**

```rust
use async_trait::async_trait;
use mockall::automock;
use std::time::Duration;

#[automock]
#[async_trait]
pub trait AiClient: Send + Sync {
    async fn chat(&self, messages: Vec<String>) -> Result<String, String>;
    async fn health_check(&self) -> bool;
}

#[automock]
#[async_trait]
pub trait ConversationStorage: Send + Sync {
    async fn add_message(&self, user_id: &str, role: &str, content: &str) -> Result<(), String>;
    async fn get_messages(&self, user_id: &str) -> Result<Vec<String>, String>;
    async fn clear(&self, user_id: &str) -> Result<bool, String>;
}

#[automock]
#[async_trait]
pub trait SignalMessenger: Send + Sync {
    async fn receive(&self) -> Result<Vec<String>, String>;
    async fn send(&self, recipient: &str, message: &str) -> Result<(), String>;
    async fn health_check(&self) -> bool;
}

#[automock]
#[async_trait]
pub trait TeeClient: Send + Sync {
    async fn is_in_tee(&self) -> bool;
    async fn get_attestation(&self) -> Result<String, String>;
}
```

---

## Test Execution Commands

### Run All Tests
```bash
# Unit tests
cargo test --workspace

# With output
cargo test --workspace -- --nocapture

# Single crate
cargo test -p near-ai-client
cargo test -p conversation-store
cargo test -p signal-client
cargo test -p signal-bot

# Integration tests only
cargo test --test '*'

# With coverage (requires cargo-tarpaulin)
cargo tarpaulin --workspace --out Html
```

### Docker-based Testing
```bash
# Start test dependencies
docker-compose -f docker/docker-compose.test.yaml up -d

# Run tests
cargo test --workspace

# Cleanup
docker-compose -f docker/docker-compose.test.yaml down
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest

    services:
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy --workspace -- -D warnings

      - name: Run tests
        run: cargo test --workspace
        env:
          REDIS_URL: redis://localhost:6379

      - name: Build release
        run: cargo build --release
```

---

## Test Coverage Goals

| Crate | Target Coverage | Priority |
|-------|-----------------|----------|
| near-ai-client | 80% | High |
| conversation-store | 85% | High |
| signal-client | 75% | Medium |
| dstack-client | 60% | Medium |
| signal-bot | 70% | High |

---

## Test Prioritization

### Must Have (Before Production)
1. ✅ near-ai-client: Response parsing tests
2. ✅ near-ai-client: Error handling tests (rate limit, auth)
3. ✅ conversation-store: CRUD operations
4. ✅ conversation-store: Trimming logic
5. ✅ signal-client: Message parsing
6. ✅ Command handler matching logic

### Should Have
1. Integration tests with mocked services
2. Property-based tests for edge cases
3. Concurrent access tests for Redis

### Nice to Have
1. End-to-end tests with real Signal
2. Performance/load tests
3. Chaos engineering tests

---

## Security Test Cases

### API Key Handling
- [ ] API key never appears in logs
- [ ] API key not serializable
- [ ] API key cleared from memory after use

### Input Validation
- [ ] Phone number format validation
- [ ] Message length limits
- [ ] UTF-8 handling for all inputs
- [ ] Markdown injection prevention

### Rate Limiting
- [ ] Per-user rate limits enforced
- [ ] Global rate limits enforced
- [ ] Rate limit responses handled correctly

---

## Estimated Effort

| Phase | Tasks | Effort |
|-------|-------|--------|
| Critical Fixes | 3 fixes | 2 hours |
| Unit Tests | ~30 tests | 4 hours |
| Integration Tests | ~10 tests | 3 hours |
| Property Tests | ~5 tests | 2 hours |
| CI/CD Setup | 1 workflow | 1 hour |
| **Total** | | **12 hours** |

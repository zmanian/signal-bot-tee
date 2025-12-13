# Implementation Plan: Signal Bot TEE (Rust)

This document outlines the detailed implementation plan for the Signal → TEE → NEAR AI Cloud private AI chat proxy, implemented in Rust for optimal TEE performance and security.

## Table of Contents

1. [Why Rust](#1-why-rust)
2. [Project Structure](#2-project-structure)
3. [Phase 1: Foundation](#3-phase-1-foundation)
4. [Phase 2: Core Components](#4-phase-2-core-components)
5. [Phase 3: Signal Integration](#5-phase-3-signal-integration)
6. [Phase 4: Bot Commands](#6-phase-4-bot-commands)
7. [Phase 5: TEE Integration](#7-phase-5-tee-integration)
8. [Phase 6: Docker & Deployment](#8-phase-6-docker--deployment)
9. [Phase 7: Testing](#9-phase-7-testing)
10. [Phase 8: Documentation & Polish](#10-phase-8-documentation--polish)
11. [File Manifest](#11-file-manifest)
12. [Dependencies](#12-dependencies)

---

## 1. Why Rust

| Benefit | Impact on This Project |
|---------|----------------------|
| Memory safety | No buffer overflows in security-critical TEE code |
| Small binary | ~15MB static binary vs ~300MB Node/Python |
| Fast startup | <50ms cold start for TEE attestation |
| No GC pauses | Predictable latency for real-time chat |
| Minimal deps | Smaller attack surface, easier audits |
| Single binary | Simple deployment, reproducible builds |
| `cargo audit` | Built-in supply chain security |

---

## 2. Project Structure

```
signal-bot-tee/
├── Cargo.toml                     # Workspace root
├── Cargo.lock                     # Locked dependencies
├── rust-toolchain.toml            # Rust version pinning
│
├── crates/
│   ├── signal-bot/                # Main application binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs            # Entry point
│   │       ├── config.rs          # Configuration
│   │       ├── error.rs           # Error types
│   │       └── commands/
│   │           ├── mod.rs
│   │           ├── chat.rs        # Chat handler
│   │           ├── verify.rs      # Attestation
│   │           ├── clear.rs       # Clear history
│   │           ├── help.rs        # Help command
│   │           └── models.rs      # List models
│   │
│   ├── near-ai-client/            # NEAR AI SDK wrapper
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs          # OpenAI-compatible client
│   │       ├── types.rs           # Request/response types
│   │       └── error.rs           # Client errors
│   │
│   ├── conversation-store/        # Redis conversation storage
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── store.rs           # Redis operations
│   │       ├── types.rs           # Message/Conversation types
│   │       └── error.rs           # Storage errors
│   │
│   ├── dstack-client/             # Dstack TEE client
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs          # Guest agent client
│   │       ├── types.rs           # Quote, AppInfo types
│   │       └── error.rs           # TEE errors
│   │
│   └── signal-client/             # Signal REST API client
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── client.rs          # HTTP client
│           ├── types.rs           # Message types
│           ├── receiver.rs        # Message polling/websocket
│           └── error.rs           # Signal errors
│
├── tests/                         # Integration tests
│   ├── common/
│   │   └── mod.rs                 # Test utilities
│   ├── near_ai_test.rs
│   ├── conversation_test.rs
│   └── e2e_test.rs
│
├── scripts/
│   ├── setup_signal.sh            # Signal account setup
│   ├── encrypt_secrets.sh         # Dstack encryption
│   └── verify_tee.sh              # TEE verification
│
├── docker/
│   ├── Dockerfile                 # Multi-stage production build
│   ├── Dockerfile.dev             # Development container
│   └── docker-compose.yaml        # Full stack
│
├── .cargo/
│   └── config.toml                # Cargo configuration
│
├── .env.example                   # Environment template
├── DESIGN.md                      # Architecture design
├── IMPLEMENTATION_PLAN.md         # This file
└── README.md                      # Project overview
```

---

## 3. Phase 1: Foundation

**Goal**: Set up Rust workspace, dependencies, configuration, and error handling.

### 3.1 Workspace Root

**File: `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/signal-bot",
    "crates/near-ai-client",
    "crates/conversation-store",
    "crates/dstack-client",
    "crates/signal-client",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT"
repository = "https://github.com/example/signal-bot-tee"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }

# HTTP client
reqwest = { version = "0.11", default-features = false, features = [
    "json", "rustls-tls", "stream"
] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Redis
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Configuration
config = "0.14"
dotenvy = "0.15"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Testing
tokio-test = "0.4"
mockall = "0.12"
wiremock = "0.5"
testcontainers = "0.15"

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true

[profile.release-debug]
inherits = "release"
debug = true
strip = false
```

### 3.2 Rust Toolchain

**File: `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.75"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-musl"]
```

### 3.3 Cargo Configuration

**File: `.cargo/config.toml`**

```toml
[build]
# Static linking for TEE deployment
target = "x86_64-unknown-linux-musl"

[target.x86_64-unknown-linux-musl]
linker = "rust-lld"
rustflags = ["-C", "target-feature=+crt-static"]

[env]
# Optimize for size in release
CARGO_PROFILE_RELEASE_OPT_LEVEL = "z"

[alias]
# Convenient aliases
dev = "run --package signal-bot"
t = "test --workspace"
lint = "clippy --workspace --all-targets -- -D warnings"
```

### 3.4 Main Application Cargo.toml

**File: `crates/signal-bot/Cargo.toml`**

```toml
[package]
name = "signal-bot"
version.workspace = true
edition.workspace = true

[[bin]]
name = "signal-bot"
path = "src/main.rs"

[dependencies]
# Workspace crates
near-ai-client = { path = "../near-ai-client" }
conversation-store = { path = "../conversation-store" }
dstack-client = { path = "../dstack-client" }
signal-client = { path = "../signal-client" }

# Workspace dependencies
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
config.workspace = true
dotenvy.workspace = true
chrono.workspace = true

[dev-dependencies]
tokio-test.workspace = true
mockall.workspace = true
```

### 3.5 Configuration Module

**File: `crates/signal-bot/src/config.rs`**

```rust
//! Application configuration loaded from environment variables.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

/// Application configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Signal configuration
    #[serde(default)]
    pub signal: SignalConfig,

    /// NEAR AI configuration
    pub near_ai: NearAiConfig,

    /// Redis configuration
    #[serde(default)]
    pub redis: RedisConfig,

    /// Bot configuration
    #[serde(default)]
    pub bot: BotConfig,

    /// Dstack configuration
    #[serde(default)]
    pub dstack: DstackConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignalConfig {
    /// Signal CLI REST API endpoint
    #[serde(default = "default_signal_service")]
    pub service_url: String,

    /// Phone number for Signal bot
    pub phone_number: String,

    /// Poll interval for messages
    #[serde(default = "default_poll_interval", with = "humantime_serde")]
    pub poll_interval: Duration,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NearAiConfig {
    /// NEAR AI API key
    pub api_key: String,

    /// API base URL
    #[serde(default = "default_near_ai_url")]
    pub base_url: String,

    /// Default model
    #[serde(default = "default_model")]
    pub model: String,

    /// Request timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,

    /// Max retries
    #[serde(default = "default_retries")]
    pub max_retries: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Conversation TTL
    #[serde(default = "default_ttl", with = "humantime_serde")]
    pub ttl: Duration,

    /// Max messages per conversation
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    /// System prompt for AI
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DstackConfig {
    /// Dstack guest agent socket path
    #[serde(default = "default_dstack_socket")]
    pub socket_path: String,
}

// Default implementations
impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            service_url: default_signal_service(),
            phone_number: String::new(),
            poll_interval: default_poll_interval(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            ttl: default_ttl(),
            max_messages: default_max_messages(),
        }
    }
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            system_prompt: default_system_prompt(),
            log_level: default_log_level(),
        }
    }
}

impl Default for DstackConfig {
    fn default() -> Self {
        Self {
            socket_path: default_dstack_socket(),
        }
    }
}

// Default value functions
fn default_signal_service() -> String {
    "http://signal-api:8080".into()
}

fn default_poll_interval() -> Duration {
    Duration::from_secs(1)
}

fn default_near_ai_url() -> String {
    "https://api.near.ai/v1".into()
}

fn default_model() -> String {
    "llama-3.3-70b".into()
}

fn default_timeout() -> Duration {
    Duration::from_secs(60)
}

fn default_retries() -> u32 {
    3
}

fn default_redis_url() -> String {
    "redis://localhost:6379".into()
}

fn default_ttl() -> Duration {
    Duration::from_secs(24 * 60 * 60) // 24 hours
}

fn default_max_messages() -> usize {
    50
}

fn default_system_prompt() -> String {
    "You are a helpful AI assistant accessible via Signal. \
     You provide accurate, thoughtful responses while being concise for mobile chat. \
     You're running in a privacy-preserving environment with verifiable execution."
        .into()
}

fn default_log_level() -> String {
    "info".into()
}

fn default_dstack_socket() -> String {
    "/var/run/dstack.sock".into()
}

impl Config {
    /// Load configuration from environment variables.
    pub fn load() -> Result<Self> {
        // Load .env file if present
        dotenvy::dotenv().ok();

        let config = config::Config::builder()
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .try_parsing(true),
            )
            .build()
            .context("Failed to build configuration")?;

        config
            .try_deserialize()
            .context("Failed to deserialize configuration")
    }
}
```

### 3.6 Error Types

**File: `crates/signal-bot/src/error.rs`**

```rust
//! Application error types.

use thiserror::Error;

/// Main application error type.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] anyhow::Error),

    #[error("Signal error: {0}")]
    Signal(#[from] signal_client::SignalError),

    #[error("NEAR AI error: {0}")]
    NearAi(#[from] near_ai_client::NearAiError),

    #[error("Conversation error: {0}")]
    Conversation(#[from] conversation_store::ConversationError),

    #[error("Dstack error: {0}")]
    Dstack(#[from] dstack_client::DstackError),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for application errors.
pub type AppResult<T> = Result<T, AppError>;
```

### 3.7 Tasks for Phase 1

| Task | Description | Files |
|------|-------------|-------|
| 1.1 | Create workspace Cargo.toml | `Cargo.toml` |
| 1.2 | Set up rust-toolchain.toml | `rust-toolchain.toml` |
| 1.3 | Configure cargo for static builds | `.cargo/config.toml` |
| 1.4 | Create signal-bot crate | `crates/signal-bot/` |
| 1.5 | Implement configuration | `crates/signal-bot/src/config.rs` |
| 1.6 | Define error types | `crates/signal-bot/src/error.rs` |
| 1.7 | Set up tracing/logging | `crates/signal-bot/src/main.rs` |
| 1.8 | Create .env.example | `.env.example` |

---

## 4. Phase 2: Core Components

**Goal**: Implement the core client libraries as separate crates.

### 4.1 NEAR AI Client

**File: `crates/near-ai-client/Cargo.toml`**

```toml
[package]
name = "near-ai-client"
version.workspace = true
edition.workspace = true

[dependencies]
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true

# Async streaming
futures = "0.3"
tokio-stream = "0.1"

[dev-dependencies]
tokio-test.workspace = true
wiremock.workspace = true
```

**File: `crates/near-ai-client/src/lib.rs`**

```rust
//! NEAR AI Cloud client with OpenAI-compatible API.

mod client;
mod error;
mod types;

pub use client::NearAiClient;
pub use error::NearAiError;
pub use types::*;
```

**File: `crates/near-ai-client/src/error.rs`**

```rust
//! NEAR AI client errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NearAiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Authentication failed")]
    Unauthorized,

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Stream error: {0}")]
    Stream(String),
}
```

**File: `crates/near-ai-client/src/types.rs`**

```rust
//! Request and response types for NEAR AI API.

use serde::{Deserialize, Serialize};

/// Chat message role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Chat completion request.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Chat completion response.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming chunk response.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Delta {
    pub role: Option<Role>,
    pub content: Option<String>,
}

/// Model information.
#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

/// Models list response.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<Model>,
}

/// Attestation report from NEAR AI.
#[derive(Debug, Clone, Deserialize)]
pub struct AttestationReport {
    #[serde(flatten)]
    pub extra: serde_json::Value,
}
```

**File: `crates/near-ai-client/src/client.rs`**

```rust
//! NEAR AI Cloud HTTP client.

use crate::error::NearAiError;
use crate::types::*;
use futures::StreamExt;
use reqwest::{Client, StatusCode};
use std::time::Duration;
use tokio_stream::Stream;
use tracing::{debug, instrument, warn};

/// NEAR AI Cloud client.
#[derive(Clone)]
pub struct NearAiClient {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl NearAiClient {
    /// Create a new NEAR AI client.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, NearAiError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        })
    }

    /// Get the configured model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Send a chat completion request.
    #[instrument(skip(self, messages), fields(message_count = messages.len()))]
    pub async fn chat(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<String, NearAiError> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            max_tokens,
            stream: Some(false),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        self.handle_response::<ChatResponse>(response)
            .await
            .map(|r| {
                r.choices
                    .into_iter()
                    .next()
                    .map(|c| c.message.content)
                    .unwrap_or_default()
            })
    }

    /// Send a streaming chat completion request.
    #[instrument(skip(self, messages), fields(message_count = messages.len()))]
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<impl Stream<Item = Result<String, NearAiError>>, NearAiError> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            max_tokens,
            stream: Some(true),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(self.extract_error(response).await);
        }

        let stream = response.bytes_stream().map(|result| {
            result
                .map_err(NearAiError::from)
                .and_then(|bytes| {
                    // Parse SSE data
                    let text = String::from_utf8_lossy(&bytes);
                    let mut content = String::new();

                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                continue;
                            }
                            if let Ok(chunk) = serde_json::from_str::<ChatChunk>(data) {
                                if let Some(delta_content) = chunk
                                    .choices
                                    .first()
                                    .and_then(|c| c.delta.content.as_ref())
                                {
                                    content.push_str(delta_content);
                                }
                            }
                        }
                    }

                    Ok(content)
                })
        });

        Ok(stream)
    }

    /// List available models.
    #[instrument(skip(self))]
    pub async fn list_models(&self) -> Result<Vec<Model>, NearAiError> {
        let response = self
            .client
            .get(format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        self.handle_response::<ModelsResponse>(response)
            .await
            .map(|r| r.data)
    }

    /// Get attestation report from NEAR AI.
    #[instrument(skip(self))]
    pub async fn get_attestation(&self) -> Result<AttestationReport, NearAiError> {
        let response = self
            .client
            .get(format!("{}/attestation", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Health check - returns true if API is reachable.
    pub async fn health_check(&self) -> bool {
        self.list_models().await.is_ok()
    }

    /// Handle HTTP response, converting errors appropriately.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, NearAiError> {
        let status = response.status();

        if status.is_success() {
            let body = response.text().await?;
            debug!("Response body: {}", &body[..body.len().min(200)]);
            serde_json::from_str(&body).map_err(NearAiError::from)
        } else {
            Err(self.extract_error(response).await)
        }
    }

    /// Extract error information from failed response.
    async fn extract_error(&self, response: reqwest::Response) -> NearAiError {
        let status = response.status();

        match status {
            StatusCode::TOO_MANY_REQUESTS => {
                warn!("Rate limit exceeded");
                NearAiError::RateLimit
            }
            StatusCode::UNAUTHORIZED => {
                warn!("Authentication failed");
                NearAiError::Unauthorized
            }
            _ => {
                let message = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".into());
                NearAiError::Api {
                    status: status.as_u16(),
                    message,
                }
            }
        }
    }
}
```

### 4.2 Conversation Store

**File: `crates/conversation-store/Cargo.toml`**

```toml
[package]
name = "conversation-store"
version.workspace = true
edition.workspace = true

[dependencies]
redis.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono.workspace = true

[dev-dependencies]
tokio-test.workspace = true
testcontainers.workspace = true
```

**File: `crates/conversation-store/src/lib.rs`**

```rust
//! Redis-backed conversation storage.

mod error;
mod store;
mod types;

pub use error::ConversationError;
pub use store::ConversationStore;
pub use types::*;
```

**File: `crates/conversation-store/src/error.rs`**

```rust
//! Conversation storage errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConversationError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Not connected to Redis")]
    NotConnected,
}
```

**File: `crates/conversation-store/src/types.rs`**

```rust
//! Conversation and message types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl StoredMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
        }
    }
}

/// A conversation with a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub user_id: String,
    pub messages: Vec<StoredMessage>,
    pub system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    pub fn new(user_id: impl Into<String>, system_prompt: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            user_id: user_id.into(),
            messages: Vec::new(),
            system_prompt,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a message to the conversation.
    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(StoredMessage::new(role, content));
        self.updated_at = Utc::now();
    }

    /// Trim to max messages, keeping most recent.
    pub fn trim(&mut self, max_messages: usize) {
        if self.messages.len() > max_messages {
            let start = self.messages.len() - max_messages;
            self.messages = self.messages[start..].to_vec();
        }
    }
}

/// OpenAI-compatible message format.
#[derive(Debug, Clone, Serialize)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: String,
}
```

**File: `crates/conversation-store/src/store.rs`**

```rust
//! Redis conversation storage implementation.

use crate::error::ConversationError;
use crate::types::*;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::time::Duration;
use tracing::{debug, info, instrument};

/// Redis-backed conversation store.
#[derive(Clone)]
pub struct ConversationStore {
    conn: ConnectionManager,
    max_messages: usize,
    ttl: Duration,
}

impl ConversationStore {
    /// Create a new conversation store.
    pub async fn new(
        redis_url: &str,
        max_messages: usize,
        ttl: Duration,
    ) -> Result<Self, ConversationError> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;

        info!("Connected to Redis at {}", redis_url);

        Ok(Self {
            conn,
            max_messages,
            ttl,
        })
    }

    /// Generate Redis key for a user.
    fn key(&self, user_id: &str) -> String {
        format!("conversation:{}", user_id)
    }

    /// Get conversation for a user.
    #[instrument(skip(self))]
    pub async fn get(&self, user_id: &str) -> Result<Option<Conversation>, ConversationError> {
        let mut conn = self.conn.clone();
        let data: Option<String> = conn.get(self.key(user_id)).await?;

        match data {
            Some(json) => {
                let conv: Conversation = serde_json::from_str(&json)?;
                debug!("Retrieved conversation with {} messages", conv.messages.len());
                Ok(Some(conv))
            }
            None => Ok(None),
        }
    }

    /// Add a message to a conversation, creating if needed.
    #[instrument(skip(self, content))]
    pub async fn add_message(
        &self,
        user_id: &str,
        role: &str,
        content: &str,
        system_prompt: Option<&str>,
    ) -> Result<Conversation, ConversationError> {
        let mut conv = self
            .get(user_id)
            .await?
            .unwrap_or_else(|| Conversation::new(user_id, system_prompt.map(String::from)));

        // Update system prompt if provided
        if let Some(prompt) = system_prompt {
            conv.system_prompt = Some(prompt.to_string());
        }

        // Add the message
        conv.add_message(role, content);

        // Trim old messages
        conv.trim(self.max_messages);

        // Save to Redis
        self.save(&conv).await?;

        Ok(conv)
    }

    /// Save a conversation to Redis.
    async fn save(&self, conv: &Conversation) -> Result<(), ConversationError> {
        let mut conn = self.conn.clone();
        let json = serde_json::to_string(conv)?;
        let ttl_secs = self.ttl.as_secs() as i64;

        conn.set_ex(self.key(&conv.user_id), json, ttl_secs as u64)
            .await?;

        debug!("Saved conversation for {}", conv.user_id);
        Ok(())
    }

    /// Clear a user's conversation.
    #[instrument(skip(self))]
    pub async fn clear(&self, user_id: &str) -> Result<bool, ConversationError> {
        let mut conn = self.conn.clone();
        let deleted: i64 = conn.del(self.key(user_id)).await?;

        if deleted > 0 {
            info!("Cleared conversation for {}", user_id);
        }

        Ok(deleted > 0)
    }

    /// Convert conversation to OpenAI messages format.
    pub async fn to_openai_messages(
        &self,
        user_id: &str,
        system_prompt: Option<&str>,
    ) -> Result<Vec<OpenAiMessage>, ConversationError> {
        let conv = self.get(user_id).await?;
        let mut messages = Vec::new();

        // Add system prompt
        let prompt = system_prompt
            .map(String::from)
            .or_else(|| conv.as_ref().and_then(|c| c.system_prompt.clone()));

        if let Some(p) = prompt {
            messages.push(OpenAiMessage {
                role: "system".into(),
                content: p,
            });
        }

        // Add conversation history
        if let Some(conv) = conv {
            for msg in conv.messages {
                messages.push(OpenAiMessage {
                    role: msg.role,
                    content: msg.content,
                });
            }
        }

        Ok(messages)
    }

    /// Get message count for a user.
    pub async fn message_count(&self, user_id: &str) -> Result<usize, ConversationError> {
        Ok(self
            .get(user_id)
            .await?
            .map(|c| c.messages.len())
            .unwrap_or(0))
    }
}
```

### 4.3 Dstack Client

**File: `crates/dstack-client/Cargo.toml`**

```toml
[package]
name = "dstack-client"
version.workspace = true
edition.workspace = true

[dependencies]
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true

# Unix socket support
hyper = { version = "0.14", features = ["client", "http1"] }
hyperlocal = "0.8"
hex = "0.4"

[dev-dependencies]
tokio-test.workspace = true
```

**File: `crates/dstack-client/src/lib.rs`**

```rust
//! Dstack TEE guest agent client.

mod client;
mod error;
mod types;

pub use client::DstackClient;
pub use error::DstackError;
pub use types::*;
```

**File: `crates/dstack-client/src/error.rs`**

```rust
//! Dstack client errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DstackError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Hyper error: {0}")]
    Hyper(#[from] hyper::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Quote generation failed: {0}")]
    QuoteGeneration(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Not running in TEE")]
    NotInTee,

    #[error("Socket not found: {0}")]
    SocketNotFound(String),
}
```

**File: `crates/dstack-client/src/types.rs`**

```rust
//! Dstack API types.

use serde::{Deserialize, Serialize};

/// Application information from Dstack.
#[derive(Debug, Clone, Deserialize)]
pub struct AppInfo {
    /// Unique application identifier
    pub app_id: Option<String>,

    /// Hash of docker-compose configuration
    pub compose_hash: Option<String>,

    /// Unique instance identifier
    pub instance_id: Option<String>,

    /// Additional fields
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// TDX attestation quote.
#[derive(Debug, Clone, Deserialize)]
pub struct Quote {
    /// Base64-encoded TDX quote
    pub quote: String,

    /// Report data that was included
    pub report_data: Option<String>,
}

/// Key derivation request.
#[derive(Debug, Clone, Serialize)]
pub struct DeriveKeyRequest {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// Key derivation response.
#[derive(Debug, Clone, Deserialize)]
pub struct DeriveKeyResponse {
    /// Hex-encoded derived key
    pub key: String,
}

/// RA-TLS certificate response.
#[derive(Debug, Clone, Deserialize)]
pub struct RaTlsCert {
    /// Base64-encoded certificate
    pub cert: String,
}
```

**File: `crates/dstack-client/src/client.rs`**

```rust
//! Dstack guest agent client implementation.

use crate::error::DstackError;
use crate::types::*;
use hyper::{Body, Client, Method, Request};
use hyperlocal::{UnixClientExt, Uri};
use std::path::Path;
use tracing::{debug, instrument, warn};

/// Client for Dstack guest agent.
pub struct DstackClient {
    socket_path: String,
}

impl DstackClient {
    /// Create a new Dstack client.
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    /// Check if running inside a TEE.
    pub async fn is_in_tee(&self) -> bool {
        if !Path::new(&self.socket_path).exists() {
            return false;
        }
        self.get_app_info().await.is_ok()
    }

    /// Get application information.
    #[instrument(skip(self))]
    pub async fn get_app_info(&self) -> Result<AppInfo, DstackError> {
        let response = self.request(Method::GET, "/Info", None).await?;
        let info: AppInfo = serde_json::from_slice(&response)?;
        debug!("Got app info: {:?}", info);
        Ok(info)
    }

    /// Generate TDX attestation quote.
    #[instrument(skip(self, report_data))]
    pub async fn get_quote(&self, report_data: &[u8]) -> Result<Quote, DstackError> {
        // Pad or truncate report_data to 64 bytes
        let mut data = [0u8; 64];
        let len = report_data.len().min(64);
        data[..len].copy_from_slice(&report_data[..len]);

        let hex_data = hex::encode(data);
        let path = format!("/GetQuote?report_data={}", hex_data);

        let response = self.request(Method::GET, &path, None).await?;
        let quote: Quote = serde_json::from_slice(&response)?;

        debug!("Generated quote with {} bytes", quote.quote.len());
        Ok(quote)
    }

    /// Derive a key from TEE root of trust.
    #[instrument(skip(self))]
    pub async fn derive_key(
        &self,
        path: &str,
        subject: Option<&str>,
    ) -> Result<Vec<u8>, DstackError> {
        let request = DeriveKeyRequest {
            path: path.to_string(),
            subject: subject.map(String::from),
        };

        let body = serde_json::to_vec(&request)?;
        let response = self.request(Method::POST, "/DeriveKey", Some(body)).await?;
        let result: DeriveKeyResponse = serde_json::from_slice(&response)?;

        hex::decode(&result.key).map_err(|e| DstackError::KeyDerivation(e.to_string()))
    }

    /// Get RA-TLS certificate.
    #[instrument(skip(self))]
    pub async fn get_ra_tls_cert(&self) -> Result<Vec<u8>, DstackError> {
        let response = self.request(Method::GET, "/GetRaTlsCert", None).await?;
        let cert: RaTlsCert = serde_json::from_slice(&response)?;

        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD
            .decode(&cert.cert)
            .map_err(|e| DstackError::QuoteGeneration(e.to_string()))
    }

    /// Make HTTP request to Dstack socket.
    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, DstackError> {
        if !Path::new(&self.socket_path).exists() {
            return Err(DstackError::SocketNotFound(self.socket_path.clone()));
        }

        let client = Client::unix();
        let uri = Uri::new(&self.socket_path, path);

        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header("Content-Type", "application/json");

        let body = match body {
            Some(b) => Body::from(b),
            None => Body::empty(),
        };

        let request = request.body(body).map_err(|e| {
            DstackError::QuoteGeneration(format!("Failed to build request: {}", e))
        })?;

        let response = client.request(request).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = hyper::body::to_bytes(response.into_body()).await?;
            let msg = String::from_utf8_lossy(&body);
            warn!("Dstack request failed: {} - {}", status, msg);
            return Err(DstackError::QuoteGeneration(format!(
                "HTTP {}: {}",
                status, msg
            )));
        }

        let body = hyper::body::to_bytes(response.into_body()).await?;
        Ok(body.to_vec())
    }
}
```

### 4.4 Tasks for Phase 2

| Task | Description | Files |
|------|-------------|-------|
| 2.1 | Create near-ai-client crate | `crates/near-ai-client/` |
| 2.2 | Implement chat completion | `client.rs` |
| 2.3 | Implement streaming | `client.rs` |
| 2.4 | Create conversation-store crate | `crates/conversation-store/` |
| 2.5 | Implement Redis storage | `store.rs` |
| 2.6 | Create dstack-client crate | `crates/dstack-client/` |
| 2.7 | Implement Unix socket client | `client.rs` |
| 2.8 | Implement quote generation | `client.rs` |
| 2.9 | Write unit tests | `tests/` |

---

## 5. Phase 3: Signal Integration

**Goal**: Implement Signal CLI REST API client.

### 5.1 Signal Client

**File: `crates/signal-client/Cargo.toml`**

```toml
[package]
name = "signal-client"
version.workspace = true
edition.workspace = true

[dependencies]
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono.workspace = true

# Async channels
tokio-stream = "0.1"
async-stream = "0.3"

[dev-dependencies]
tokio-test.workspace = true
wiremock.workspace = true
```

**File: `crates/signal-client/src/lib.rs`**

```rust
//! Signal CLI REST API client.

mod client;
mod error;
mod receiver;
mod types;

pub use client::SignalClient;
pub use error::SignalError;
pub use receiver::MessageReceiver;
pub use types::*;
```

**File: `crates/signal-client/src/error.rs`**

```rust
//! Signal client errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignalError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Not registered")]
    NotRegistered,

    #[error("Send failed: {0}")]
    SendFailed(String),
}
```

**File: `crates/signal-client/src/types.rs`**

```rust
//! Signal API types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Incoming Signal message.
#[derive(Debug, Clone, Deserialize)]
pub struct IncomingMessage {
    pub envelope: Envelope,
    pub account: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    pub source: String,
    #[serde(rename = "sourceNumber")]
    pub source_number: Option<String>,
    #[serde(rename = "sourceName")]
    pub source_name: Option<String>,
    pub timestamp: i64,
    #[serde(rename = "dataMessage")]
    pub data_message: Option<DataMessage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DataMessage {
    pub message: Option<String>,
    pub timestamp: i64,
    #[serde(rename = "groupInfo")]
    pub group_info: Option<GroupInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupInfo {
    #[serde(rename = "groupId")]
    pub group_id: String,
}

/// Outgoing message request.
#[derive(Debug, Clone, Serialize)]
pub struct SendMessageRequest {
    pub message: String,
    pub number: Option<String>,
    pub recipients: Option<Vec<String>>,
}

/// Send message response.
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageResponse {
    pub timestamp: Option<i64>,
}

/// Account information.
#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub number: String,
    pub uuid: Option<String>,
    pub registered: bool,
}

/// Parsed message for bot processing.
#[derive(Debug, Clone)]
pub struct BotMessage {
    pub source: String,
    pub text: String,
    pub timestamp: i64,
    pub is_group: bool,
    pub group_id: Option<String>,
}

impl BotMessage {
    /// Extract bot message from incoming envelope.
    pub fn from_incoming(msg: &IncomingMessage) -> Option<Self> {
        let data = msg.envelope.data_message.as_ref()?;
        let text = data.message.clone()?;

        Some(Self {
            source: msg.envelope.source.clone(),
            text,
            timestamp: msg.envelope.timestamp,
            is_group: data.group_info.is_some(),
            group_id: data.group_info.as_ref().map(|g| g.group_id.clone()),
        })
    }

    /// Get the reply target (group ID or source number).
    pub fn reply_target(&self) -> &str {
        self.group_id.as_deref().unwrap_or(&self.source)
    }
}
```

**File: `crates/signal-client/src/client.rs`**

```rust
//! Signal HTTP client.

use crate::error::SignalError;
use crate::types::*;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

/// Signal CLI REST API client.
#[derive(Clone)]
pub struct SignalClient {
    client: Client,
    base_url: String,
    phone_number: String,
}

impl SignalClient {
    /// Create a new Signal client.
    pub fn new(
        base_url: impl Into<String>,
        phone_number: impl Into<String>,
    ) -> Result<Self, SignalError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            phone_number: phone_number.into(),
        })
    }

    /// Get the configured phone number.
    pub fn phone_number(&self) -> &str {
        &self.phone_number
    }

    /// Check if the Signal API is healthy.
    pub async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/v1/health", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Get account information.
    #[instrument(skip(self))]
    pub async fn get_account(&self) -> Result<Account, SignalError> {
        let response = self
            .client
            .get(format!("{}/v1/accounts/{}", self.base_url, self.phone_number))
            .send()
            .await?;

        if !response.status().is_success() {
            let msg = response.text().await.unwrap_or_default();
            return Err(SignalError::Api(msg));
        }

        Ok(response.json().await?)
    }

    /// Receive pending messages.
    #[instrument(skip(self))]
    pub async fn receive(&self) -> Result<Vec<IncomingMessage>, SignalError> {
        let response = self
            .client
            .get(format!(
                "{}/v1/receive/{}",
                self.base_url, self.phone_number
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            let msg = response.text().await.unwrap_or_default();
            return Err(SignalError::Api(msg));
        }

        let messages: Vec<IncomingMessage> = response.json().await?;
        debug!("Received {} messages", messages.len());
        Ok(messages)
    }

    /// Send a message to a recipient.
    #[instrument(skip(self, message))]
    pub async fn send(&self, recipient: &str, message: &str) -> Result<(), SignalError> {
        let request = SendMessageRequest {
            message: message.to_string(),
            number: Some(self.phone_number.clone()),
            recipients: Some(vec![recipient.to_string()]),
        };

        let response = self
            .client
            .post(format!(
                "{}/v2/send/{}",
                self.base_url, self.phone_number
            ))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let msg = response.text().await.unwrap_or_default();
            warn!("Send failed: {}", msg);
            return Err(SignalError::SendFailed(msg));
        }

        debug!("Sent message to {}", recipient);
        Ok(())
    }

    /// Reply to a message (handles both direct and group messages).
    pub async fn reply(&self, original: &BotMessage, message: &str) -> Result<(), SignalError> {
        self.send(original.reply_target(), message).await
    }
}
```

**File: `crates/signal-client/src/receiver.rs`**

```rust
//! Message receiver with polling.

use crate::client::SignalClient;
use crate::error::SignalError;
use crate::types::*;
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::Stream;
use tracing::{debug, error, warn};

/// Message receiver that polls for new messages.
pub struct MessageReceiver {
    client: SignalClient,
    poll_interval: Duration,
}

impl MessageReceiver {
    /// Create a new message receiver.
    pub fn new(client: SignalClient, poll_interval: Duration) -> Self {
        Self {
            client,
            poll_interval,
        }
    }

    /// Start receiving messages as an async stream.
    pub fn stream(self) -> impl Stream<Item = BotMessage> {
        async_stream::stream! {
            loop {
                match self.client.receive().await {
                    Ok(messages) => {
                        for msg in messages {
                            if let Some(bot_msg) = BotMessage::from_incoming(&msg) {
                                debug!("Received: {} from {}",
                                    &bot_msg.text[..bot_msg.text.len().min(50)],
                                    bot_msg.source
                                );
                                yield bot_msg;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Receive error: {}", e);
                        // Back off on error
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                }

                sleep(self.poll_interval).await;
            }
        }
    }
}
```

### 5.2 Tasks for Phase 3

| Task | Description | Files |
|------|-------------|-------|
| 3.1 | Create signal-client crate | `crates/signal-client/` |
| 3.2 | Implement message types | `types.rs` |
| 3.3 | Implement HTTP client | `client.rs` |
| 3.4 | Implement message receiver | `receiver.rs` |
| 3.5 | Write integration tests | `tests/` |

---

## 6. Phase 4: Bot Commands

**Goal**: Implement command handlers and message routing.

### 6.1 Commands Module

**File: `crates/signal-bot/src/commands/mod.rs`**

```rust
//! Bot command handlers.

mod chat;
mod clear;
mod help;
mod models;
mod verify;

pub use chat::ChatHandler;
pub use clear::ClearHandler;
pub use help::HelpHandler;
pub use models::ModelsHandler;
pub use verify::VerifyHandler;

use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;

/// Command handler trait.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Command name (e.g., "help", "clear").
    fn name(&self) -> &str;

    /// Command trigger (e.g., "!help").
    fn trigger(&self) -> Option<&str> {
        None
    }

    /// Whether this is the default handler for non-command messages.
    fn is_default(&self) -> bool {
        false
    }

    /// Check if this handler matches the message.
    fn matches(&self, message: &BotMessage) -> bool {
        if let Some(trigger) = self.trigger() {
            message.text.starts_with(trigger)
        } else {
            self.is_default() && !message.text.starts_with('!')
        }
    }

    /// Execute the command.
    async fn execute(&self, message: &BotMessage) -> AppResult<String>;
}
```

### 6.2 Chat Handler

**File: `crates/signal-bot/src/commands/chat.rs`**

```rust
//! Chat command - proxies messages to NEAR AI.

use crate::commands::CommandHandler;
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use conversation_store::ConversationStore;
use near_ai_client::{Message, NearAiClient, NearAiError};
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::{error, info, instrument};

pub struct ChatHandler {
    near_ai: Arc<NearAiClient>,
    conversations: Arc<ConversationStore>,
    system_prompt: String,
}

impl ChatHandler {
    pub fn new(
        near_ai: Arc<NearAiClient>,
        conversations: Arc<ConversationStore>,
        system_prompt: String,
    ) -> Self {
        Self {
            near_ai,
            conversations,
            system_prompt,
        }
    }
}

#[async_trait]
impl CommandHandler for ChatHandler {
    fn name(&self) -> &str {
        "chat"
    }

    fn is_default(&self) -> bool {
        true
    }

    #[instrument(skip(self, message), fields(user = %message.source))]
    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let user_id = &message.source;

        info!(
            "Chat from {}: {}...",
            &user_id[..user_id.len().min(8)],
            &message.text[..message.text.len().min(50)]
        );

        // Add user message to history
        self.conversations
            .add_message(user_id, "user", &message.text, Some(&self.system_prompt))
            .await?;

        // Get full conversation for context
        let stored_messages = self
            .conversations
            .to_openai_messages(user_id, Some(&self.system_prompt))
            .await?;

        // Convert to NEAR AI message format
        let messages: Vec<Message> = stored_messages
            .into_iter()
            .map(|m| Message {
                role: match m.role.as_str() {
                    "system" => near_ai_client::Role::System,
                    "assistant" => near_ai_client::Role::Assistant,
                    _ => near_ai_client::Role::User,
                },
                content: m.content,
            })
            .collect();

        // Query NEAR AI
        let response = match self.near_ai.chat(messages, Some(0.7), None).await {
            Ok(r) => r,
            Err(NearAiError::RateLimit) => {
                return Ok(
                    "⏳ I'm receiving too many requests. Please wait a moment and try again."
                        .into(),
                );
            }
            Err(e) => {
                error!("NEAR AI error: {}", e);
                return Ok(
                    "Sorry, I encountered an error connecting to the AI service. Please try again."
                        .into(),
                );
            }
        };

        // Store assistant response
        self.conversations
            .add_message(user_id, "assistant", &response, None)
            .await?;

        info!(
            "Response to {}: {} chars",
            &user_id[..user_id.len().min(8)],
            response.len()
        );

        Ok(response)
    }
}
```

### 6.3 Verify Handler

**File: `crates/signal-bot/src/commands/verify.rs`**

```rust
//! Verify command - provides attestation proofs.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use dstack_client::DstackClient;
use near_ai_client::NearAiClient;
use sha2::{Digest, Sha256};
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::{info, warn};

pub struct VerifyHandler {
    near_ai: Arc<NearAiClient>,
    dstack: Arc<DstackClient>,
}

impl VerifyHandler {
    pub fn new(near_ai: Arc<NearAiClient>, dstack: Arc<DstackClient>) -> Self {
        Self { near_ai, dstack }
    }

    async fn get_proxy_info(&self, challenge: &[u8]) -> ProxyInfo {
        if !self.dstack.is_in_tee().await {
            return ProxyInfo {
                available: false,
                reason: Some("Not running in TEE environment".into()),
                ..Default::default()
            };
        }

        match self.dstack.get_app_info().await {
            Ok(info) => {
                let quote_ok = self.dstack.get_quote(challenge).await.is_ok();
                ProxyInfo {
                    available: true,
                    compose_hash: info.compose_hash,
                    app_id: info.app_id,
                    quote_generated: quote_ok,
                    reason: None,
                }
            }
            Err(e) => ProxyInfo {
                available: false,
                reason: Some(e.to_string()),
                ..Default::default()
            },
        }
    }

    async fn get_near_info(&self) -> NearInfo {
        match self.near_ai.get_attestation().await {
            Ok(_attestation) => NearInfo {
                available: true,
                model: self.near_ai.model().to_string(),
                reason: None,
            },
            Err(e) => NearInfo {
                available: false,
                model: self.near_ai.model().to_string(),
                reason: Some(e.to_string()),
            },
        }
    }

    fn format_response(&self, proxy: ProxyInfo, near: NearInfo) -> String {
        let mut lines = vec!["🔐 **Privacy Verification**".to_string(), String::new()];

        // Proxy section
        lines.push("**Proxy (Signal Bot)**".into());
        if proxy.available {
            lines.push("├─ TEE: Intel TDX".into());
            if let Some(hash) = &proxy.compose_hash {
                lines.push(format!("├─ Compose Hash: {}...", &hash[..hash.len().min(16)]));
            }
            if let Some(id) = &proxy.app_id {
                lines.push(format!("├─ App ID: {}...", &id[..id.len().min(16)]));
            }
            lines.push("└─ Verify: https://proof.phala.network".into());
        } else {
            lines.push(format!(
                "└─ ⚠️ {}",
                proxy.reason.unwrap_or("Unavailable".into())
            ));
        }

        lines.push(String::new());

        // Inference section
        lines.push("**Inference (NEAR AI Cloud)**".into());
        if near.available {
            lines.push("├─ TEE: NVIDIA GPU (H100/H200)".into());
            lines.push(format!("├─ Model: {}", near.model));
            lines.push("├─ Gateway: Intel TDX".into());
            lines.push("└─ Verify: https://near.ai/verify".into());
        } else {
            lines.push(format!("├─ Model: {}", near.model));
            lines.push(format!(
                "└─ ⚠️ {}",
                near.reason.unwrap_or("Unavailable".into())
            ));
        }

        lines.push(String::new());
        lines.push("Both layers provide hardware-backed attestation.".into());
        lines.push("Your messages never exist in plaintext outside TEEs.".into());

        lines.join("\n")
    }
}

#[derive(Default)]
struct ProxyInfo {
    available: bool,
    compose_hash: Option<String>,
    app_id: Option<String>,
    quote_generated: bool,
    reason: Option<String>,
}

struct NearInfo {
    available: bool,
    model: String,
    reason: Option<String>,
}

#[async_trait]
impl CommandHandler for VerifyHandler {
    fn name(&self) -> &str {
        "verify"
    }

    fn trigger(&self) -> Option<&str> {
        Some("!verify")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        info!("Attestation requested by {}", message.source);

        // Generate challenge
        let mut hasher = Sha256::new();
        hasher.update(message.timestamp.to_string().as_bytes());
        hasher.update(message.source.as_bytes());
        let challenge = hasher.finalize();

        let proxy = self.get_proxy_info(&challenge).await;
        let near = self.get_near_info().await;

        Ok(self.format_response(proxy, near))
    }
}
```

### 6.4 Other Commands

**File: `crates/signal-bot/src/commands/clear.rs`**

```rust
//! Clear command - resets conversation history.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use conversation_store::ConversationStore;
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::info;

pub struct ClearHandler {
    conversations: Arc<ConversationStore>,
}

impl ClearHandler {
    pub fn new(conversations: Arc<ConversationStore>) -> Self {
        Self { conversations }
    }
}

#[async_trait]
impl CommandHandler for ClearHandler {
    fn name(&self) -> &str {
        "clear"
    }

    fn trigger(&self) -> Option<&str> {
        Some("!clear")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let cleared = self.conversations.clear(&message.source).await?;

        if cleared {
            info!("Cleared history for {}", &message.source[..8.min(message.source.len())]);
            Ok("✅ Conversation history cleared.".into())
        } else {
            Ok("No conversation history to clear.".into())
        }
    }
}
```

**File: `crates/signal-bot/src/commands/help.rs`**

```rust
//! Help command - displays available commands.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;

pub struct HelpHandler;

impl HelpHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for HelpHandler {
    fn name(&self) -> &str {
        "help"
    }

    fn trigger(&self) -> Option<&str> {
        Some("!help")
    }

    async fn execute(&self, _message: &BotMessage) -> AppResult<String> {
        Ok(r#"🤖 **Signal AI** (Private & Verifiable)

Just send a message to chat with AI.

**Commands:**
• !verify - Show privacy attestation proofs
• !clear - Clear conversation history
• !models - List available AI models
• !help - Show this message

**Privacy:**
Your messages are end-to-end encrypted via Signal, processed in a verified TEE (Intel TDX), and sent to NEAR AI Cloud's private inference (NVIDIA GPU TEE).

Neither the bot operator nor NEAR AI can read your messages."#
            .into())
    }
}
```

**File: `crates/signal-bot/src/commands/models.rs`**

```rust
//! Models command - lists available AI models.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use near_ai_client::NearAiClient;
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::error;

pub struct ModelsHandler {
    near_ai: Arc<NearAiClient>,
}

impl ModelsHandler {
    pub fn new(near_ai: Arc<NearAiClient>) -> Self {
        Self { near_ai }
    }
}

#[async_trait]
impl CommandHandler for ModelsHandler {
    fn name(&self) -> &str {
        "models"
    }

    fn trigger(&self) -> Option<&str> {
        Some("!models")
    }

    async fn execute(&self, _message: &BotMessage) -> AppResult<String> {
        match self.near_ai.list_models().await {
            Ok(models) => {
                let model_list: String = models
                    .iter()
                    .take(10)
                    .map(|m| format!("• {}", m.id))
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(format!(
                    "**Available Models:**\n{}\n\n_Current: {}_",
                    model_list,
                    self.near_ai.model()
                ))
            }
            Err(e) => {
                error!("Failed to list models: {}", e);
                Ok("❌ Could not fetch model list.".into())
            }
        }
    }
}
```

### 6.5 Tasks for Phase 4

| Task | Description | Files |
|------|-------------|-------|
| 4.1 | Create commands module | `commands/mod.rs` |
| 4.2 | Implement ChatHandler | `commands/chat.rs` |
| 4.3 | Implement VerifyHandler | `commands/verify.rs` |
| 4.4 | Implement ClearHandler | `commands/clear.rs` |
| 4.5 | Implement HelpHandler | `commands/help.rs` |
| 4.6 | Implement ModelsHandler | `commands/models.rs` |
| 4.7 | Write command tests | `tests/` |

---

## 7. Phase 5: TEE Integration

**Goal**: Main application entry point and TEE verification.

### 7.1 Main Entry Point

**File: `crates/signal-bot/src/main.rs`**

```rust
//! Signal AI Proxy Bot - Main entry point.

mod commands;
mod config;
mod error;

use crate::commands::*;
use crate::config::Config;
use crate::error::AppResult;
use anyhow::Context;
use conversation_store::ConversationStore;
use dstack_client::DstackClient;
use near_ai_client::NearAiClient;
use signal_client::{MessageReceiver, SignalClient};
use std::sync::Arc;
use tokio::signal;
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> AppResult<()> {
    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Initialize logging
    init_logging(&config.bot.log_level);

    info!("Starting Signal AI Proxy Bot...");

    // Initialize clients
    let near_ai = Arc::new(
        NearAiClient::new(
            &config.near_ai.api_key,
            &config.near_ai.base_url,
            &config.near_ai.model,
            config.near_ai.timeout,
        )
        .context("Failed to create NEAR AI client")?,
    );

    let conversations = Arc::new(
        ConversationStore::new(
            &config.redis.url,
            config.redis.max_messages,
            config.redis.ttl,
        )
        .await
        .context("Failed to connect to Redis")?,
    );

    let dstack = Arc::new(DstackClient::new(&config.dstack.socket_path));

    let signal = SignalClient::new(&config.signal.service_url, &config.signal.phone_number)
        .context("Failed to create Signal client")?;

    // Health checks
    if near_ai.health_check().await {
        info!("NEAR AI healthy - Model: {}", config.near_ai.model);
    } else {
        warn!("NEAR AI health check failed - will retry on requests");
    }

    if dstack.is_in_tee().await {
        if let Ok(info) = dstack.get_app_info().await {
            info!(
                "Running in TEE - App ID: {}",
                info.app_id.as_deref().unwrap_or("unknown")
            );
        }
    } else {
        warn!("Not running in TEE environment - attestation unavailable");
    }

    if !signal.health_check().await {
        error!("Signal API not reachable at {}", config.signal.service_url);
        return Err(anyhow::anyhow!("Signal API not reachable").into());
    }
    info!("Signal API healthy");

    // Create command handlers
    let handlers: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(ChatHandler::new(
            near_ai.clone(),
            conversations.clone(),
            config.bot.system_prompt.clone(),
        )),
        Box::new(VerifyHandler::new(near_ai.clone(), dstack.clone())),
        Box::new(ClearHandler::new(conversations.clone())),
        Box::new(HelpHandler::new()),
        Box::new(ModelsHandler::new(near_ai.clone())),
    ];

    info!("Registered {} command handlers", handlers.len());
    info!("NEAR AI endpoint: {}", config.near_ai.base_url);
    info!("Listening for messages...");

    // Start message receiver
    let receiver = MessageReceiver::new(signal.clone(), config.signal.poll_interval);
    let mut stream = Box::pin(receiver.stream());

    // Main message loop
    loop {
        tokio::select! {
            Some(message) = stream.next() => {
                // Find matching handler
                let handler = handlers
                    .iter()
                    .find(|h| h.matches(&message));

                if let Some(handler) = handler {
                    match handler.execute(&message).await {
                        Ok(response) => {
                            if let Err(e) = signal.reply(&message, &response).await {
                                error!("Failed to send reply: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Handler error: {}", e);
                            let _ = signal
                                .reply(&message, "Sorry, something went wrong.")
                                .await;
                        }
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    info!("Shutting down...");
    Ok(())
}

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
```

### 7.2 Tasks for Phase 5

| Task | Description | Files |
|------|-------------|-------|
| 5.1 | Implement main entry point | `main.rs` |
| 5.2 | Add graceful shutdown | `main.rs` |
| 5.3 | Add health checks | `main.rs` |
| 5.4 | Test locally | Manual |
| 5.5 | Test in TEE | Manual |

---

## 8. Phase 6: Docker & Deployment

**Goal**: Production-ready container and deployment configuration.

### 8.1 Production Dockerfile

**File: `docker/Dockerfile`**

```dockerfile
# Build stage
FROM rust:1.75-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/signal-bot/Cargo.toml crates/signal-bot/
COPY crates/near-ai-client/Cargo.toml crates/near-ai-client/
COPY crates/conversation-store/Cargo.toml crates/conversation-store/
COPY crates/dstack-client/Cargo.toml crates/dstack-client/
COPY crates/signal-client/Cargo.toml crates/signal-client/

# Create dummy source files for dependency caching
RUN mkdir -p crates/signal-bot/src && echo "fn main() {}" > crates/signal-bot/src/main.rs
RUN mkdir -p crates/near-ai-client/src && echo "" > crates/near-ai-client/src/lib.rs
RUN mkdir -p crates/conversation-store/src && echo "" > crates/conversation-store/src/lib.rs
RUN mkdir -p crates/dstack-client/src && echo "" > crates/dstack-client/src/lib.rs
RUN mkdir -p crates/signal-client/src && echo "" > crates/signal-client/src/lib.rs

# Build dependencies only
RUN cargo build --release --target x86_64-unknown-linux-musl 2>/dev/null || true

# Copy actual source
COPY crates crates

# Touch source files to trigger rebuild
RUN find crates -name "*.rs" -exec touch {} \;

# Build release binary
RUN cargo build --release --target x86_64-unknown-linux-musl --bin signal-bot

# Runtime stage
FROM scratch

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/signal-bot /signal-bot

# Non-root user (UID 1000)
USER 1000

ENTRYPOINT ["/signal-bot"]
```

### 8.2 Docker Compose

**File: `docker/docker-compose.yaml`**

```yaml
version: "3.8"

services:
  signal-api:
    image: bbernhard/signal-cli-rest-api:latest
    container_name: signal-api
    environment:
      - MODE=json-rpc
      - JSON_RPC_TRUST_NEW_IDENTITIES=on-first-use
      - LOG_LEVEL=info
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock:ro
      - signal-config:/home/.local/share/signal-cli
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
    restart: unless-stopped

  signal-bot:
    build:
      context: ..
      dockerfile: docker/Dockerfile
    container_name: signal-bot
    environment:
      - SIGNAL__SERVICE_URL=http://signal-api:8080
      - SIGNAL__PHONE_NUMBER=${SIGNAL_PHONE}
      - NEAR_AI__API_KEY=${NEAR_AI_API_KEY}
      - NEAR_AI__BASE_URL=${NEAR_AI_BASE_URL:-https://api.near.ai/v1}
      - NEAR_AI__MODEL=${NEAR_AI_MODEL:-llama-3.3-70b}
      - REDIS__URL=redis://redis:6379
      - BOT__LOG_LEVEL=${LOG_LEVEL:-info}
      - DSTACK__SOCKET_PATH=/var/run/dstack.sock
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock:ro
    depends_on:
      signal-api:
        condition: service_healthy
      redis:
        condition: service_started
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    container_name: redis
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3
    restart: unless-stopped

volumes:
  signal-config:
    driver: local
  redis-data:
    driver: local

networks:
  default:
    name: signal-bot-network
```

### 8.3 Environment Template

**File: `.env.example`**

```bash
# Signal Configuration
SIGNAL_PHONE=+1234567890

# NEAR AI Configuration
NEAR_AI_API_KEY=your-api-key-here
NEAR_AI_BASE_URL=https://api.near.ai/v1
NEAR_AI_MODEL=llama-3.3-70b

# Logging
LOG_LEVEL=info
```

### 8.4 Tasks for Phase 6

| Task | Description | Files |
|------|-------------|-------|
| 6.1 | Create production Dockerfile | `docker/Dockerfile` |
| 6.2 | Create docker-compose.yaml | `docker/docker-compose.yaml` |
| 6.3 | Create .env.example | `.env.example` |
| 6.4 | Create setup scripts | `scripts/` |
| 6.5 | Test local deployment | Manual |
| 6.6 | Test Dstack deployment | Manual |
| 6.7 | Verify binary size <20MB | Manual |

---

## 9. Phase 7: Testing

**Goal**: Comprehensive test coverage.

### 9.1 Test Configuration

**File: `tests/common/mod.rs`**

```rust
//! Common test utilities.

use near_ai_client::NearAiClient;
use conversation_store::ConversationStore;
use std::sync::Arc;
use wiremock::MockServer;

pub async fn mock_near_ai_server() -> MockServer {
    MockServer::start().await
}

pub fn test_near_ai_client(mock_server: &MockServer) -> NearAiClient {
    NearAiClient::new(
        "test-api-key",
        &mock_server.uri(),
        "test-model",
        std::time::Duration::from_secs(5),
    )
    .unwrap()
}
```

### 9.2 Tasks for Phase 7

| Task | Description | Files |
|------|-------------|-------|
| 7.1 | Create test utilities | `tests/common/` |
| 7.2 | Write near-ai-client tests | `tests/near_ai_test.rs` |
| 7.3 | Write conversation tests | `tests/conversation_test.rs` |
| 7.4 | Write command tests | Unit tests in crates |
| 7.5 | Write integration tests | `tests/e2e_test.rs` |
| 7.6 | Achieve >80% coverage | All tests |

---

## 10. Phase 8: Documentation & Polish

| Task | Description | Files |
|------|-------------|-------|
| 8.1 | Update README | `README.md` |
| 8.2 | Add rustdoc comments | All source files |
| 8.3 | Run clippy and fix warnings | All source files |
| 8.4 | Security audit with cargo-audit | N/A |
| 8.5 | Create CHANGELOG | `CHANGELOG.md` |

---

## 11. File Manifest

### Crates
- `crates/signal-bot/src/{main,config,error}.rs`
- `crates/signal-bot/src/commands/{mod,chat,verify,clear,help,models}.rs`
- `crates/near-ai-client/src/{lib,client,types,error}.rs`
- `crates/conversation-store/src/{lib,store,types,error}.rs`
- `crates/dstack-client/src/{lib,client,types,error}.rs`
- `crates/signal-client/src/{lib,client,types,error,receiver}.rs`

### Configuration
- `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`
- `.env.example`

### Docker
- `docker/Dockerfile`, `docker/docker-compose.yaml`

### Scripts
- `scripts/setup_signal.sh`, `scripts/encrypt_secrets.sh`

### Tests
- `tests/common/mod.rs`
- `tests/{near_ai,conversation,e2e}_test.rs`

---

## 12. Dependencies

### Production Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.35 | Async runtime |
| reqwest | 0.11 | HTTP client |
| serde | 1.0 | Serialization |
| redis | 0.24 | Redis client |
| tracing | 0.1 | Logging |
| thiserror | 1.0 | Error types |
| anyhow | 1.0 | Error handling |
| chrono | 0.4 | Time handling |
| hyper | 0.14 | Unix socket HTTP |
| hyperlocal | 0.8 | Unix socket support |

### Dev Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio-test | 0.4 | Async testing |
| mockall | 0.12 | Mocking |
| wiremock | 0.5 | HTTP mocking |
| testcontainers | 0.15 | Container testing |

---

## Summary

This Rust implementation provides:

| Metric | Value |
|--------|-------|
| Total tasks | ~50 |
| Crates | 5 |
| Binary size | ~15MB (static musl) |
| Memory usage | ~10-30MB |
| Cold start | <50ms |
| Dependencies | ~60 crates |

**Key advantages over Python:**
- 10x smaller deployment footprint
- 3-5x lower memory usage
- Near-instant startup
- Compile-time safety
- Smaller attack surface

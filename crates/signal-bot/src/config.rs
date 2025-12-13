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

    /// Conversation storage configuration
    #[serde(default)]
    pub conversation: ConversationConfig,

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConversationConfig {
    /// Conversation TTL (how long before inactive conversations expire)
    #[serde(default = "default_ttl", with = "humantime_serde")]
    pub ttl: Duration,

    /// Max messages per conversation (older messages are trimmed)
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

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
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

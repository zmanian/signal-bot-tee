//! Configuration for the registration proxy.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

/// Proxy configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Signal API configuration
    #[serde(default)]
    pub signal: SignalConfig,

    /// Registry storage configuration
    #[serde(default)]
    pub registry: RegistryConfig,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Dstack configuration
    #[serde(default)]
    pub dstack: DstackConfig,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: RateLimitConfig,

    /// Logging configuration
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignalConfig {
    /// Signal CLI REST API URL
    #[serde(default = "default_signal_api_url")]
    pub api_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryConfig {
    /// Path to encrypted registry file
    #[serde(default = "default_registry_path")]
    pub path: PathBuf,

    /// Enable persistence (if false, registry is in-memory only)
    #[serde(default = "default_true")]
    pub persist: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Server listen address
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DstackConfig {
    /// Dstack socket path
    #[serde(default = "default_dstack_socket")]
    pub socket_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Global requests per minute
    #[serde(default = "default_global_rpm")]
    pub global_per_minute: u32,

    /// Per-phone-number requests per hour
    #[serde(default = "default_per_number_rph")]
    pub per_number_per_hour: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
}

// Default implementations
impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            api_url: default_signal_api_url(),
        }
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            path: default_registry_path(),
            persist: true,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            port: default_port(),
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

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            global_per_minute: default_global_rpm(),
            per_number_per_hour: default_per_number_rph(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

// Default value functions
fn default_signal_api_url() -> String {
    "http://signal-api:8080".into()
}

fn default_registry_path() -> PathBuf {
    PathBuf::from("/data/registry.enc")
}

fn default_true() -> bool {
    true
}

fn default_listen_addr() -> String {
    "0.0.0.0".into()
}

fn default_port() -> u16 {
    8081
}

fn default_dstack_socket() -> String {
    "/var/run/dstack.sock".into()
}

fn default_global_rpm() -> u32 {
    10
}

fn default_per_number_rph() -> u32 {
    3
}

fn default_log_level() -> String {
    "info".into()
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
                    .try_parsing(false),
            )
            .build()
            .context("Failed to build configuration")?;

        config
            .try_deserialize()
            .context("Failed to deserialize configuration")
    }
}

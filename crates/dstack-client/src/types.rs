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

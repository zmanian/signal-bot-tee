//! API request and response types.

use crate::registry::RegistrationStatus;
use serde::{Deserialize, Serialize};

/// Request to initiate phone number registration.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Optional CAPTCHA token if required by Signal
    pub captcha: Option<String>,

    /// Use voice call instead of SMS for verification code
    #[serde(default)]
    pub use_voice: bool,

    /// Optional ownership proof secret (will be hashed and stored)
    /// Required for later unregistration or re-registration
    pub ownership_secret: Option<String>,
}

/// Response after initiating registration.
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub phone_number: String,
    pub status: String,
    pub message: String,
}

/// Request to verify registration with code.
#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    /// Optional Signal PIN to set
    pub pin: Option<String>,

    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Response after verification.
#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub phone_number: String,
    pub status: String,
    pub message: String,
}

/// Phone number status response.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub phone_number: String,
    pub status: RegistrationStatus,
    pub registered_at: Option<String>,
}

/// List of registered accounts.
#[derive(Debug, Serialize)]
pub struct AccountsResponse {
    pub accounts: Vec<AccountInfo>,
    pub total: usize,
}

/// Account info for listing.
#[derive(Debug, Serialize)]
pub struct AccountInfo {
    pub phone_number: String,
    pub status: RegistrationStatus,
    pub registered_at: String,
}

/// Request to unregister a number.
#[derive(Debug, Deserialize)]
pub struct UnregisterRequest {
    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub registry_count: usize,
    pub signal_api_healthy: bool,
}

//! API request/response types.

use crate::types::{Chain, DepositStatus};
use serde::{Deserialize, Serialize};

/// Balance response.
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub user_id: String,
    pub credits_remaining: u64,
    /// Human-readable USDC amount.
    pub credits_remaining_usdc: String,
    pub total_deposited_usdc: String,
    pub total_consumed_usdc: String,
}

/// Deposit request.
#[derive(Debug, Serialize, Deserialize)]
pub struct DepositRequest {
    /// Chain where deposit was made.
    pub chain: Chain,
    /// Transaction hash or payment payload.
    pub tx_hash: String,
    /// User's phone number (E.164 format).
    pub user_id: String,
    /// Amount claimed in micro-USDC.
    pub amount: u64,
}

/// Deposit response.
#[derive(Debug, Serialize, Deserialize)]
pub struct DepositResponse {
    pub deposit_id: String,
    pub credits_granted: u64,
    pub new_balance: u64,
    pub tx_hash: String,
    pub status: DepositStatus,
}

/// Deposit address response.
#[derive(Debug, Serialize, Deserialize)]
pub struct DepositAddressResponse {
    pub chain: Chain,
    pub address: String,
    pub token: String,
    pub token_contract: String,
    /// Memo/reference to include (for NEAR).
    pub memo: Option<String>,
}

/// Pricing response.
#[derive(Debug, Serialize, Deserialize)]
pub struct PricingResponse {
    pub prompt_cost_per_million_tokens: String,
    pub completion_cost_per_million_tokens: String,
    pub minimum_per_message: String,
    pub supported_chains: Vec<ChainInfo>,
}

/// Chain information.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain: Chain,
    pub enabled: bool,
    pub token: String,
    pub deposit_address: String,
}

/// Health check response.
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub healthy: bool,
    pub payments_enabled: bool,
    pub chains: Vec<ChainHealth>,
}

/// Health status for a chain.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainHealth {
    pub chain: Chain,
    pub enabled: bool,
    pub healthy: bool,
}

/// Error response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
        }
    }
}

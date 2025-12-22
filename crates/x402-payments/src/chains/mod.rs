//! Multi-chain payment verification and settlement.

use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Payment payload from a client.
#[derive(Debug, Clone)]
pub struct PaymentPayload {
    /// Which chain the payment is on.
    pub chain: Chain,
    /// Sender's wallet address.
    pub sender: String,
    /// Amount in micro-USDC (1e-6).
    pub amount: u64,
    /// Cryptographic signature or transaction hash.
    pub signature: String,
    /// Nonce for replay protection.
    pub nonce: u64,
    /// When this payload expires.
    pub expiry: DateTime<Utc>,
}

/// Result of verifying a payment.
#[derive(Debug, Clone)]
pub struct PaymentVerification {
    /// Whether the payment is valid.
    pub valid: bool,
    /// Sender's current balance (if available).
    pub sender_balance: Option<u64>,
    /// Error message if invalid.
    pub error: Option<String>,
}

impl PaymentVerification {
    pub fn valid() -> Self {
        Self {
            valid: true,
            sender_balance: None,
            error: None,
        }
    }

    pub fn invalid(reason: impl Into<String>) -> Self {
        Self {
            valid: false,
            sender_balance: None,
            error: Some(reason.into()),
        }
    }
}

/// Result of a transfer operation.
#[derive(Debug, Clone)]
pub struct TxResult {
    /// Transaction hash.
    pub hash: String,
    /// Block number (if confirmed).
    pub block: Option<u64>,
}

/// Chain-agnostic payment facilitator trait.
///
/// Each chain (Base, NEAR, Solana) implements this trait to provide
/// payment verification, settlement, and wallet management.
#[async_trait]
pub trait ChainFacilitator: Send + Sync {
    /// Get the chain identifier.
    fn chain(&self) -> Chain;

    /// Get the deposit address for this chain.
    ///
    /// Users send USDC to this address to add credits.
    fn deposit_address(&self) -> String;

    /// Verify a payment payload is valid and funded.
    async fn verify_payment(&self, payload: &PaymentPayload) -> Result<PaymentVerification, PaymentError>;

    /// Settle a verified payment on-chain.
    ///
    /// This submits the payment transaction and returns once it's confirmed.
    async fn settle_payment(&self, payload: &PaymentPayload) -> Result<SettlementResult, PaymentError>;

    /// Get the current USDC balance of the deposit wallet.
    ///
    /// Used by FundSweeper to know how much to sweep.
    async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError>;

    /// Transfer USDC from deposit wallet to destination.
    ///
    /// Used by FundSweeper to send funds to operator.
    async fn transfer_to(&self, destination: &str, amount: u64) -> Result<TxResult, PaymentError>;

    /// Monitor a transaction for confirmation.
    async fn get_tx_status(&self, tx_hash: &str) -> Result<TxStatus, PaymentError>;

    /// Check if this chain is currently healthy/available.
    async fn health_check(&self) -> Result<bool, PaymentError> {
        // Default implementation assumes healthy
        Ok(true)
    }
}

// Placeholder implementations for each chain.
// These will be replaced with real implementations.

pub mod base;
pub mod near;
pub mod solana;

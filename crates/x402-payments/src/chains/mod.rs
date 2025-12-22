//! Multi-chain payment verification and settlement.

use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;

/// Payment payload for deposit verification.
///
/// Used to verify that a user has deposited USDC on-chain.
#[derive(Debug, Clone)]
pub struct PaymentPayload {
    /// Which chain the payment is on.
    pub chain: Chain,
    /// Transaction hash to verify.
    pub tx_hash: String,
    /// Expected sender address (optional, for verification).
    pub from: Option<String>,
    /// Expected amount in micro-USDC (optional, for verification).
    pub amount: Option<u64>,
    /// User ID to credit (phone number).
    pub user_id: String,
}

impl PaymentPayload {
    /// Create a new payment payload for verification.
    pub fn new(chain: Chain, tx_hash: String, user_id: String) -> Self {
        Self {
            chain,
            tx_hash,
            from: None,
            amount: None,
            user_id,
        }
    }

    /// Set the expected sender address.
    pub fn with_from(mut self, from: String) -> Self {
        self.from = Some(from);
        self
    }

    /// Set the expected amount.
    pub fn with_amount(mut self, amount: u64) -> Self {
        self.amount = Some(amount);
        self
    }
}

/// Result of verifying a payment on-chain.
#[derive(Debug, Clone)]
pub struct PaymentVerification {
    /// Transaction hash that was verified.
    pub tx_hash: String,
    /// Amount in micro-USDC (1e-6).
    pub amount_usdc: u64,
    /// Sender address (if available).
    pub from: Option<String>,
    /// Recipient address (our deposit address).
    pub to: String,
    /// Number of block confirmations.
    pub confirmations: u64,
    /// Whether the payment is fully verified.
    pub verified: bool,
}

impl PaymentVerification {
    /// Check if payment has enough confirmations (default: 1).
    pub fn is_confirmed(&self, min_confirmations: u64) -> bool {
        self.verified && self.confirmations >= min_confirmations
    }
}

/// Result of a transfer operation.
#[derive(Debug, Clone)]
pub struct TxResult {
    /// Transaction hash.
    pub tx_hash: String,
    /// Block number (if confirmed).
    pub block_number: Option<u64>,
    /// Whether the transaction succeeded.
    pub success: bool,
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

    /// Verify a payment/deposit on-chain.
    ///
    /// Checks that a transaction:
    /// 1. Exists and is confirmed
    /// 2. Is a USDC transfer
    /// 3. Transfers to our deposit address
    /// 4. Matches expected amount (if specified)
    async fn verify_payment(
        &self,
        payload: &PaymentPayload,
    ) -> Result<PaymentVerification, PaymentError>;

    /// Settle a verified payment on-chain.
    ///
    /// For our deposit verification model, this is typically not used
    /// since the user already transferred funds. This would be used
    /// in a full x402 facilitator role.
    async fn settle_payment(
        &self,
        payload: &PaymentPayload,
    ) -> Result<SettlementResult, PaymentError>;

    /// Get the current USDC balance of the deposit wallet.
    ///
    /// Used by FundSweeper to know how much to sweep.
    async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError>;

    /// Transfer USDC from deposit wallet to destination.
    ///
    /// Used by FundSweeper to send funds to operator.
    async fn transfer_to(&self, destination: &str, amount: u64) -> Result<TxResult, PaymentError>;

    /// Get the status of a transaction.
    async fn get_tx_status(&self, tx_hash: &str) -> Result<TxStatus, PaymentError>;

    /// Check if this chain is currently healthy/available.
    async fn health_check(&self) -> Result<bool, PaymentError> {
        // Default implementation assumes healthy
        Ok(true)
    }
}

// Chain-specific implementations
pub mod base;
pub mod near;
pub mod solana;

// Re-exports
pub use base::BaseFacilitator;
pub use near::NearFacilitator;
pub use solana::SolanaFacilitator;

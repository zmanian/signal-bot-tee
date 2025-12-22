//! Core types for the payment system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a user (phone number in E.164 format).
pub type UserId = String;

/// Supported blockchain networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    /// Base L2 (EVM-compatible)
    Base,
    /// NEAR Protocol
    Near,
    /// Solana
    Solana,
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Chain::Base => write!(f, "Base"),
            Chain::Near => write!(f, "NEAR"),
            Chain::Solana => write!(f, "Solana"),
        }
    }
}

/// Credit balance for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditBalance {
    /// User identifier (phone number).
    pub user_id: UserId,
    /// Remaining credits in micro-units (1 USDC = 1,000,000 credits).
    pub credits_remaining: u64,
    /// Total lifetime deposits in micro-USDC.
    pub total_deposited: u64,
    /// Total lifetime consumption in micro-USDC.
    pub total_consumed: u64,
    /// Timestamp of last deposit.
    pub last_deposit_at: Option<DateTime<Utc>>,
    /// Timestamp of last usage.
    pub last_usage_at: Option<DateTime<Utc>>,
    /// When this balance was created.
    pub created_at: DateTime<Utc>,
}

impl CreditBalance {
    /// Create a new empty balance for a user.
    pub fn new(user_id: UserId) -> Self {
        Self {
            user_id,
            credits_remaining: 0,
            total_deposited: 0,
            total_consumed: 0,
            last_deposit_at: None,
            last_usage_at: None,
            created_at: Utc::now(),
        }
    }

    /// Add credits from a deposit.
    pub fn add_credits(&mut self, amount: u64) {
        self.credits_remaining = self.credits_remaining.saturating_add(amount);
        self.total_deposited = self.total_deposited.saturating_add(amount);
        self.last_deposit_at = Some(Utc::now());
    }

    /// Deduct credits for usage. Returns true if successful.
    pub fn deduct_credits(&mut self, amount: u64) -> bool {
        if self.credits_remaining >= amount {
            self.credits_remaining -= amount;
            self.total_consumed = self.total_consumed.saturating_add(amount);
            self.last_usage_at = Some(Utc::now());
            true
        } else {
            false
        }
    }

    /// Check if user has at least the specified credits.
    pub fn has_credits(&self, amount: u64) -> bool {
        self.credits_remaining >= amount
    }

    /// Convert credits to USDC (as f64 for display).
    pub fn credits_to_usdc(credits: u64) -> f64 {
        credits as f64 / 1_000_000.0
    }
}

/// Status of a deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepositStatus {
    /// Deposit is pending confirmation.
    Pending,
    /// Deposit has been confirmed and credits granted.
    Confirmed,
    /// Deposit failed or was rejected.
    Failed,
}

/// A deposit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    /// Unique deposit ID.
    pub id: String,
    /// User who made the deposit.
    pub user_id: UserId,
    /// Blockchain where deposit was made.
    pub chain: Chain,
    /// On-chain transaction hash.
    pub tx_hash: String,
    /// Amount in micro-USDC (1e-6).
    pub amount_usdc: u64,
    /// Credits granted for this deposit.
    pub credits_granted: u64,
    /// Current status.
    pub status: DepositStatus,
    /// When the deposit was initiated.
    pub created_at: DateTime<Utc>,
    /// When the deposit was confirmed (if applicable).
    pub confirmed_at: Option<DateTime<Utc>>,
}

impl Deposit {
    /// Create a new pending deposit.
    pub fn new_pending(
        user_id: UserId,
        chain: Chain,
        tx_hash: String,
        amount_usdc: u64,
        credits_granted: u64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            chain,
            tx_hash,
            amount_usdc,
            credits_granted,
            status: DepositStatus::Pending,
            created_at: Utc::now(),
            confirmed_at: None,
        }
    }

    /// Mark the deposit as confirmed.
    pub fn confirm(&mut self) {
        self.status = DepositStatus::Confirmed;
        self.confirmed_at = Some(Utc::now());
    }

    /// Mark the deposit as failed.
    pub fn fail(&mut self) {
        self.status = DepositStatus::Failed;
    }
}

/// Usage record for auditing and metering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// User who consumed credits.
    pub user_id: UserId,
    /// Conversation ID (phone number or group ID).
    pub conversation_id: String,
    /// Number of prompt tokens used.
    pub prompt_tokens: u32,
    /// Number of completion tokens used.
    pub completion_tokens: u32,
    /// Total tokens used.
    pub total_tokens: u32,
    /// Credits consumed for this usage.
    pub credits_consumed: u64,
    /// When this usage occurred.
    pub timestamp: DateTime<Utc>,
}

impl UsageRecord {
    /// Create a new usage record.
    pub fn new(
        user_id: UserId,
        conversation_id: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        credits_consumed: u64,
    ) -> Self {
        Self {
            user_id,
            conversation_id,
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            credits_consumed,
            timestamp: Utc::now(),
        }
    }
}

/// Transaction status for on-chain monitoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxStatus {
    /// Transaction is pending.
    Pending,
    /// Transaction is confirmed with N confirmations.
    Confirmed { confirmations: u64 },
    /// Transaction failed.
    Failed { reason: String },
}

/// Result of settling a payment on-chain.
#[derive(Debug, Clone)]
pub struct SettlementResult {
    /// Transaction hash.
    pub tx_hash: String,
    /// Block number (if available).
    pub block_number: Option<u64>,
    /// Whether the transaction is confirmed.
    pub confirmed: bool,
}

/// Operator addresses for fund sweeping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorAddresses {
    /// Base (EVM) operator address.
    pub base: Option<String>,
    /// NEAR operator account.
    pub near: Option<String>,
    /// Solana operator address.
    pub solana: Option<String>,
}

impl OperatorAddresses {
    /// Get the operator address for a specific chain.
    pub fn get(&self, chain: Chain) -> Option<&str> {
        match chain {
            Chain::Base => self.base.as_deref(),
            Chain::Near => self.near.as_deref(),
            Chain::Solana => self.solana.as_deref(),
        }
    }

    /// Check if any operator address is configured.
    pub fn has_any(&self) -> bool {
        self.base.is_some() || self.near.is_some() || self.solana.is_some()
    }
}

/// Record of a fund sweep operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepRecord {
    /// Chain where the sweep occurred.
    pub chain: Chain,
    /// Deposit address (source).
    pub from: String,
    /// Operator address (destination).
    pub to: String,
    /// Amount swept in micro-USDC.
    pub amount: u64,
    /// Transaction hash.
    pub tx_hash: String,
    /// Whether the sweep succeeded.
    pub success: bool,
    /// When the sweep occurred.
    pub timestamp: DateTime<Utc>,
}

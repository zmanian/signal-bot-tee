//! Payment error types.

use thiserror::Error;

/// Errors that can occur in the payment system.
#[derive(Error, Debug)]
pub enum PaymentError {
    /// Insufficient credits for the operation.
    #[error("Insufficient credits: required {required}, available {available}")]
    InsufficientCredits { required: u64, available: u64 },

    /// User not found in the credit store.
    #[error("User not found: {0}")]
    UserNotFound(String),

    /// Invalid payment payload.
    #[error("Invalid payment payload: {0}")]
    InvalidPayload(String),

    /// Payment verification failed.
    #[error("Payment verification failed: {0}")]
    VerificationFailed(String),

    /// Payment settlement failed.
    #[error("Payment settlement failed: {0}")]
    SettlementFailed(String),

    /// Transaction already processed (double-spend prevention).
    #[error("Transaction already processed: {0}")]
    DuplicateTransaction(String),

    /// Chain not supported or not enabled.
    #[error("Chain not supported: {0}")]
    UnsupportedChain(String),

    /// RPC/network error.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// RPC call failed.
    #[error("RPC error: {0}")]
    RpcError(String),

    /// Transaction not found.
    #[error("Transaction not found: {0}")]
    TxNotFound(String),

    /// Transaction failed/reverted.
    #[error("Transaction failed: {0}")]
    TxFailed(String),

    /// Invalid transaction hash.
    #[error("Invalid transaction hash: {0}")]
    InvalidTxHash(String),

    /// No transfer found in transaction.
    #[error("No transfer found: {0}")]
    NoTransferFound(String),

    /// Sender address mismatch.
    #[error("Sender mismatch: expected {expected}, got {actual}")]
    SenderMismatch { expected: String, actual: String },

    /// Amount mismatch.
    #[error("Amount mismatch: expected {expected}, got {actual}")]
    AmountMismatch { expected: u64, actual: u64 },

    /// Encryption/decryption error.
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Storage I/O error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded")]
    RateLimited,

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for PaymentError {
    fn from(e: std::io::Error) -> Self {
        PaymentError::Storage(e.to_string())
    }
}

impl From<aes_gcm::Error> for PaymentError {
    fn from(_: aes_gcm::Error) -> Self {
        PaymentError::Encryption("AES-GCM operation failed".to_string())
    }
}

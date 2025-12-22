//! x402 Payment Integration for Signal Bot TEE
//!
//! This crate provides prepaid credit payment functionality using the x402 protocol.
//! Users can deposit USDC on Base, NEAR, or Solana, and credits are deducted
//! per-message based on token usage.
//!
//! # Architecture
//!
//! ```text
//! User deposits USDC → HTTP API → Verify on-chain → Credit balance
//! User sends message → Check balance → Process → Deduct credits → Respond
//! ```
//!
//! # Modules
//!
//! - [`config`] - Payment configuration
//! - [`credits`] - Credit balance management and pricing
//! - [`chains`] - Multi-chain payment verification (Base, NEAR, Solana)
//! - [`api`] - HTTP API for deposit and balance operations
//!
//! # Security
//!
//! All credit data is encrypted at rest using TEE-derived keys via Dstack.
//! Private keys for deposit wallets are derived inside the TEE and never exposed.

pub mod api;
pub mod chains;
pub mod config;
pub mod credits;
pub mod error;
pub mod sweeper;
pub mod types;

// Re-exports for convenience
pub use config::PaymentConfig;
pub use config::PricingConfig;
pub use credits::{calculate_credits, estimate_credits, CreditStore, PricingCalculator, TokenUsage};
pub use error::PaymentError;
pub use sweeper::{spawn_sweeper, FundSweeper};
pub use types::{Chain, CreditBalance, Deposit, DepositStatus, OperatorAddresses, SweepRecord, UsageRecord};

use api::AppState;
use dstack_client::DstackClient;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

/// Start the payment HTTP server.
///
/// This creates the credit store, sets up the API router, and starts
/// listening on the configured port.
pub async fn start_payment_server(
    config: PaymentConfig,
    dstack: DstackClient,
) -> Result<(), PaymentError> {
    if !config.enabled {
        info!("Payments disabled, not starting payment server");
        return Ok(());
    }

    // Create credit store
    let credit_store = CreditStore::new(dstack, config.storage_path.clone()).await?;

    // Create app state
    // Note: Chain facilitators should be initialized and passed by the integration layer
    let state = Arc::new(AppState::new(credit_store, config.clone(), None, None, None));

    // Create router
    let router = api::create_router(state);

    // Bind to address
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        PaymentError::Internal(format!("Failed to bind to {}: {}", addr, e))
    })?;

    info!("Payment server listening on {}", addr);

    // Start server
    axum::serve(listener, router)
        .await
        .map_err(|e| PaymentError::Internal(format!("Server error: {}", e)))?;

    Ok(())
}

/// Create and run the payment server as a background task.
///
/// Returns a JoinHandle for the server task.
pub async fn spawn_payment_server(
    config: PaymentConfig,
    dstack: DstackClient,
) -> Result<Option<tokio::task::JoinHandle<Result<(), PaymentError>>>, PaymentError> {
    if !config.enabled {
        info!("Payments disabled");
        return Ok(None);
    }

    let credit_store = CreditStore::new(dstack, config.storage_path.clone()).await?;
    // Note: Chain facilitators should be initialized and passed by the integration layer
    let state = Arc::new(AppState::new(credit_store, config.clone(), None, None, None));
    let router = api::create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        PaymentError::Internal(format!("Failed to bind to {}: {}", addr, e))
    })?;

    info!("Payment server ready on {}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .map_err(|e| PaymentError::Internal(format!("Server error: {}", e)))
    });

    Ok(Some(handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PaymentConfig::default();
        assert!(config.enabled);
        assert_eq!(config.server_port, 8082);
    }
}

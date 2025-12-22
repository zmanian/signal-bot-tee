//! NEAR Protocol chain facilitator.
//!
//! Uses NEAR RPC to verify USDC transfers and manage deposits.

use super::{ChainFacilitator, PaymentPayload, PaymentVerification, TxResult};
use crate::config::NearChainConfig;
use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use tracing::warn;

/// NEAR chain facilitator.
///
/// Verifies USDC (NEP-141) transfers via NEAR RPC.
pub struct NearFacilitator {
    config: NearChainConfig,
    /// TEE-derived deposit account.
    deposit_account: String,
}

impl NearFacilitator {
    /// Create a new NEAR facilitator.
    pub async fn new(
        config: NearChainConfig,
        deposit_account: String,
    ) -> Result<Self, PaymentError> {
        // TODO: Initialize NEAR RPC client
        // let rpc = JsonRpcClient::connect(&config.rpc_url);

        Ok(Self {
            config,
            deposit_account,
        })
    }

    /// Derive deposit account from TEE key.
    ///
    /// Note: NEAR requires account creation, so this returns a subaccount
    /// of a known parent account.
    pub async fn derive_deposit_account(
        _dstack: &dstack_client::DstackClient,
    ) -> Result<String, PaymentError> {
        // TODO: Derive NEAR account ID
        // For NEAR, we might use an implicit account (hex pubkey)
        // or a named subaccount under signal-bot.near

        warn!("NEAR deposit account derivation not implemented, using placeholder");
        Ok("deposit.signal-bot.near".to_string())
    }

    /// Verify a USDC transfer transaction.
    ///
    /// Checks that:
    /// 1. Transaction exists and succeeded
    /// 2. It was a ft_transfer to our deposit account
    /// 3. The memo matches the user's phone number
    /// 4. The amount matches the claimed amount
    async fn verify_usdc_transfer(
        &self,
        _tx_hash: &str,
        _expected_sender: &str,
        _expected_amount: u64,
        _expected_memo: &str,
    ) -> Result<PaymentVerification, PaymentError> {
        // TODO: Implement NEAR transaction verification
        // 1. Query transaction status via RPC
        // 2. Parse FunctionCall to usdc_contract.ft_transfer
        // 3. Verify receiver_id, amount, and memo

        warn!("NEAR transfer verification not implemented");
        Err(PaymentError::UnsupportedChain(
            "NEAR chain not yet implemented".to_string(),
        ))
    }
}

#[async_trait]
impl ChainFacilitator for NearFacilitator {
    fn chain(&self) -> Chain {
        Chain::Near
    }

    fn deposit_address(&self) -> String {
        self.deposit_account.clone()
    }

    async fn verify_payment(
        &self,
        payload: &PaymentPayload,
    ) -> Result<PaymentVerification, PaymentError> {
        // For NEAR, the signature field contains the tx hash
        // The sender contains the NEAR account ID
        // We verify the tx was a valid ft_transfer to our deposit account

        self.verify_usdc_transfer(
            &payload.signature,
            &payload.sender,
            payload.amount,
            &payload.sender, // memo should be phone number
        )
        .await
    }

    async fn settle_payment(
        &self,
        _payload: &PaymentPayload,
    ) -> Result<SettlementResult, PaymentError> {
        // NEAR uses pull-based transfers - the user already sent the tx
        // We just verify it was received
        warn!("NEAR settlement not implemented (pull-based)");
        Err(PaymentError::UnsupportedChain(
            "NEAR chain not yet implemented".to_string(),
        ))
    }

    async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError> {
        // TODO: Query ft_balance_of for USDC on our deposit account
        warn!("NEAR balance check not implemented");
        Ok(0)
    }

    async fn transfer_to(
        &self,
        _destination: &str,
        _amount: u64,
    ) -> Result<TxResult, PaymentError> {
        // TODO: Sign and submit ft_transfer
        warn!("NEAR transfer not implemented");
        Err(PaymentError::UnsupportedChain(
            "NEAR chain not yet implemented".to_string(),
        ))
    }

    async fn get_tx_status(&self, _tx_hash: &str) -> Result<TxStatus, PaymentError> {
        // TODO: Query transaction status
        warn!("NEAR tx status not implemented");
        Ok(TxStatus::Pending)
    }

    async fn health_check(&self) -> Result<bool, PaymentError> {
        // TODO: Check NEAR RPC connectivity
        Ok(self.config.enabled)
    }
}

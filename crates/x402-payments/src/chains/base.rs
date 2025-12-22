//! Base (EVM) chain facilitator using x402-rs.
//!
//! This module will integrate with x402-rs for payment verification
//! and settlement on Base L2.

use super::{ChainFacilitator, PaymentPayload, PaymentVerification, TxResult};
use crate::config::BaseChainConfig;
use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use tracing::warn;

/// Base chain facilitator.
///
/// Uses x402-rs for EVM-compatible payment handling.
pub struct BaseFacilitator {
    config: BaseChainConfig,
    /// TEE-derived deposit wallet address.
    deposit_address: String,
}

impl BaseFacilitator {
    /// Create a new Base facilitator.
    ///
    /// The deposit address is derived from the TEE's root of trust.
    pub async fn new(
        config: BaseChainConfig,
        deposit_address: String,
    ) -> Result<Self, PaymentError> {
        // TODO: Initialize x402-rs facilitator
        // let x402_config = X402Config::from_env()?;
        // let facilitator = Facilitator::new(x402_config)?;

        Ok(Self {
            config,
            deposit_address,
        })
    }

    /// Derive deposit wallet address from TEE key.
    pub async fn derive_deposit_address(
        _dstack: &dstack_client::DstackClient,
    ) -> Result<String, PaymentError> {
        // TODO: Derive EVM address from TEE key
        // let key = dstack.derive_key("x402-payments/base-deposit-wallet", None).await?;
        // let address = eth_key_to_address(&key)?;

        warn!("Base deposit address derivation not implemented, using placeholder");
        Ok("0x0000000000000000000000000000000000000000".to_string())
    }
}

#[async_trait]
impl ChainFacilitator for BaseFacilitator {
    fn chain(&self) -> Chain {
        Chain::Base
    }

    fn deposit_address(&self) -> String {
        self.deposit_address.clone()
    }

    async fn verify_payment(
        &self,
        _payload: &PaymentPayload,
    ) -> Result<PaymentVerification, PaymentError> {
        // TODO: Use x402-rs to verify payment
        // let x402_payload = payload.into();
        // let result = self.facilitator.verify(&x402_payload).await?;

        warn!("Base payment verification not implemented");
        Err(PaymentError::UnsupportedChain(
            "Base chain not yet implemented".to_string(),
        ))
    }

    async fn settle_payment(
        &self,
        _payload: &PaymentPayload,
    ) -> Result<SettlementResult, PaymentError> {
        // TODO: Use x402-rs to settle payment
        warn!("Base payment settlement not implemented");
        Err(PaymentError::UnsupportedChain(
            "Base chain not yet implemented".to_string(),
        ))
    }

    async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError> {
        // TODO: Query USDC balance via RPC
        // let balance = self.provider.get_token_balance(&self.deposit_address, &self.config.usdc_contract).await?;

        warn!("Base balance check not implemented");
        Ok(0)
    }

    async fn transfer_to(
        &self,
        _destination: &str,
        _amount: u64,
    ) -> Result<TxResult, PaymentError> {
        // TODO: Sign and submit ERC20 transfer
        warn!("Base transfer not implemented");
        Err(PaymentError::UnsupportedChain(
            "Base chain not yet implemented".to_string(),
        ))
    }

    async fn get_tx_status(&self, _tx_hash: &str) -> Result<TxStatus, PaymentError> {
        // TODO: Query transaction receipt
        warn!("Base tx status not implemented");
        Ok(TxStatus::Pending)
    }

    async fn health_check(&self) -> Result<bool, PaymentError> {
        // TODO: Check RPC connectivity
        // Try a simple eth_blockNumber call
        Ok(self.config.enabled)
    }
}

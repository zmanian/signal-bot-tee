//! Solana chain facilitator using raw JSON-RPC.
//!
//! Verifies SPL token (USDC) transfers on Solana.

use super::{ChainFacilitator, PaymentPayload, PaymentVerification, TxResult};
use crate::config::SolanaChainConfig;
use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

/// Solana chain facilitator.
///
/// Uses Solana JSON-RPC for SPL token payment verification.
pub struct SolanaFacilitator {
    config: SolanaChainConfig,
    /// TEE-derived deposit wallet public key (base58).
    deposit_address: String,
    /// HTTP client for RPC calls.
    client: reqwest::Client,
}

/// Solana JSON-RPC request structure.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
    id: u64,
}

/// Solana JSON-RPC response structure.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

// RPC response structures - some fields unused but required for deserialization
#[allow(dead_code)]
mod rpc_types {
    use serde::Deserialize;

    /// Transaction response from getTransaction.
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TransactionResponse {
        pub slot: u64,
        pub transaction: TransactionData,
        pub meta: TransactionMeta,
        pub block_time: Option<i64>,
    }

    #[derive(Debug, Deserialize)]
    pub struct TransactionData {
        pub message: TransactionMessage,
        pub signatures: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TransactionMessage {
        pub account_keys: Vec<String>,
        pub instructions: Vec<InstructionData>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct InstructionData {
        pub program_id_index: u8,
        pub accounts: Vec<u8>,
        pub data: String, // base58 encoded
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TransactionMeta {
        pub err: Option<serde_json::Value>,
        pub fee: u64,
        pub pre_token_balances: Option<Vec<TokenBalance>>,
        pub post_token_balances: Option<Vec<TokenBalance>>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TokenBalance {
        pub account_index: u8,
        pub mint: String,
        pub owner: Option<String>,
        pub ui_token_amount: UiTokenAmount,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct UiTokenAmount {
        pub amount: String,
        pub decimals: u8,
        pub ui_amount: Option<f64>,
    }

    /// Token account balance response.
    #[derive(Debug, Deserialize)]
    pub struct TokenAccountBalance {
        pub value: TokenBalanceValue,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TokenBalanceValue {
        pub amount: String,
        pub decimals: u8,
        pub ui_amount: Option<f64>,
    }

    /// Slot info for health check.
    #[derive(Debug, Deserialize)]
    pub struct SlotInfo {
        pub slot: u64,
    }

    /// Signature status.
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SignatureStatus {
        pub slot: u64,
        pub confirmations: Option<u64>,
        pub err: Option<serde_json::Value>,
        pub confirmation_status: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct SignatureStatusResult {
        pub value: Vec<Option<SignatureStatus>>,
    }
}

use rpc_types::*;

impl SolanaFacilitator {
    /// Create a new Solana facilitator.
    pub async fn new(
        config: SolanaChainConfig,
        deposit_address: String,
    ) -> Result<Self, PaymentError> {
        info!(
            "Initializing Solana facilitator: rpc={}, usdc_mint={}, deposit={}",
            config.rpc_url, config.usdc_mint, deposit_address
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PaymentError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            deposit_address,
            client,
        })
    }

    /// Derive deposit wallet from TEE key.
    ///
    /// Derives a Solana public key from TEE-derived entropy.
    pub async fn derive_deposit_address(
        dstack: &dstack_client::DstackClient,
    ) -> Result<String, PaymentError> {
        // Derive 32-byte key from TEE
        let key_bytes = dstack
            .derive_key("x402-payments/solana-deposit-wallet", None)
            .await
            .map_err(|e| PaymentError::Internal(format!("Failed to derive Solana key: {}", e)))?;

        if key_bytes.len() < 32 {
            return Err(PaymentError::Internal(format!(
                "Derived key too short: {} bytes",
                key_bytes.len()
            )));
        }

        // Hash to get a deterministic 32-byte key
        let mut hasher = Sha256::new();
        hasher.update(&key_bytes[..32]);
        let hash = hasher.finalize();

        // Encode as base58 (Solana public key format)
        let address = bs58::encode(&hash).into_string();
        info!("Derived Solana deposit address: {}", address);

        Ok(address)
    }

    /// Make a JSON-RPC call to Solana.
    async fn rpc_call<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        method: &'static str,
        params: T,
    ) -> Result<R, PaymentError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method,
            params,
            id: 1,
        };

        let response = self
            .client
            .post(&self.config.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| PaymentError::RpcError(format!("Solana RPC request failed: {}", e)))?;

        let json_response: JsonRpcResponse<R> = response
            .json()
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to parse Solana RPC response: {}", e)))?;

        if let Some(error) = json_response.error {
            return Err(PaymentError::RpcError(error.message));
        }

        json_response
            .result
            .ok_or_else(|| PaymentError::RpcError("Empty Solana RPC response".to_string()))
    }

    /// Get transaction details.
    async fn get_transaction(&self, signature: &str) -> Result<Option<TransactionResponse>, PaymentError> {
        let params = serde_json::json!([
            signature,
            {
                "encoding": "json",
                "maxSupportedTransactionVersion": 0
            }
        ]);

        self.rpc_call("getTransaction", params).await
    }

    /// Get token account balance (SPL token).
    #[allow(dead_code)] // Will be used when ATA derivation is implemented
    async fn get_token_account_balance(&self, token_account: &str) -> Result<u64, PaymentError> {
        let params = serde_json::json!([token_account]);

        let result: TokenAccountBalance = self.rpc_call("getTokenAccountBalance", params).await?;

        result
            .value
            .amount
            .parse::<u64>()
            .map_err(|e| PaymentError::Internal(format!("Invalid balance format: {}", e)))
    }

    /// Get signature statuses.
    async fn get_signature_statuses(&self, signatures: &[&str]) -> Result<SignatureStatusResult, PaymentError> {
        let params = serde_json::json!([signatures, {"searchTransactionHistory": true}]);
        self.rpc_call("getSignatureStatuses", params).await
    }

    /// Verify a USDC transfer transaction.
    async fn verify_usdc_transfer(
        &self,
        signature: &str,
        expected_from: Option<&str>,
        expected_amount: Option<u64>,
    ) -> Result<PaymentVerification, PaymentError> {
        // Get transaction details
        let tx = self
            .get_transaction(signature)
            .await?
            .ok_or_else(|| PaymentError::TxNotFound(signature.to_string()))?;

        // Check transaction succeeded
        if tx.meta.err.is_some() {
            return Err(PaymentError::TxFailed(format!(
                "Transaction failed: {:?}",
                tx.meta.err
            )));
        }

        // Analyze token balance changes
        let pre_balances = tx.meta.pre_token_balances.unwrap_or_default();
        let post_balances = tx.meta.post_token_balances.unwrap_or_default();

        let mut verified_amount: u64 = 0;
        let mut verified_from: Option<String> = None;

        // Find USDC balance changes to our deposit address
        for post in &post_balances {
            // Check if this is USDC
            if post.mint != self.config.usdc_mint {
                continue;
            }

            // Check if this is our deposit address
            let owner = post.owner.as_deref().unwrap_or("");
            if owner != self.deposit_address {
                continue;
            }

            // Find corresponding pre-balance
            let pre_amount: u64 = pre_balances
                .iter()
                .find(|p| p.account_index == post.account_index)
                .map(|p| p.ui_token_amount.amount.parse().unwrap_or(0))
                .unwrap_or(0);

            let post_amount: u64 = post.ui_token_amount.amount.parse().unwrap_or(0);

            if post_amount > pre_amount {
                verified_amount = post_amount - pre_amount;

                // Try to find the sender
                for pre in &pre_balances {
                    if pre.mint == self.config.usdc_mint {
                        let pre_bal: u64 = pre.ui_token_amount.amount.parse().unwrap_or(0);
                        if let Some(post_entry) = post_balances
                            .iter()
                            .find(|p| p.account_index == pre.account_index)
                        {
                            let post_bal: u64 = post_entry.ui_token_amount.amount.parse().unwrap_or(0);
                            if pre_bal > post_bal {
                                verified_from = pre.owner.clone();
                                break;
                            }
                        }
                    }
                }
                break;
            }
        }

        if verified_amount == 0 {
            return Err(PaymentError::NoTransferFound(format!(
                "No USDC transfer to {} found in tx {}",
                self.deposit_address, signature
            )));
        }

        // Verify sender if expected
        if let Some(expected) = expected_from {
            if let Some(ref actual) = verified_from {
                if actual != expected {
                    return Err(PaymentError::SenderMismatch {
                        expected: expected.to_string(),
                        actual: actual.clone(),
                    });
                }
            }
        }

        // Verify amount if expected
        if let Some(expected) = expected_amount {
            if verified_amount != expected {
                return Err(PaymentError::AmountMismatch {
                    expected,
                    actual: verified_amount,
                });
            }
        }

        debug!(
            "Verified Solana USDC transfer: from={:?}, amount={}, signature={}",
            verified_from, verified_amount, signature
        );

        Ok(PaymentVerification {
            tx_hash: signature.to_string(),
            amount_usdc: verified_amount,
            from: verified_from,
            to: self.deposit_address.clone(),
            confirmations: 1, // Solana finality is fast
            verified: true,
        })
    }
}

#[async_trait]
impl ChainFacilitator for SolanaFacilitator {
    fn chain(&self) -> Chain {
        Chain::Solana
    }

    fn deposit_address(&self) -> String {
        self.deposit_address.clone()
    }

    async fn verify_payment(
        &self,
        payload: &PaymentPayload,
    ) -> Result<PaymentVerification, PaymentError> {
        // tx_hash contains the Solana transaction signature
        self.verify_usdc_transfer(&payload.tx_hash, payload.from.as_deref(), payload.amount)
            .await
    }

    async fn settle_payment(
        &self,
        _payload: &PaymentPayload,
    ) -> Result<SettlementResult, PaymentError> {
        // For deposit verification model, settlement is not needed
        Err(PaymentError::UnsupportedChain(
            "Settlement not required for deposit verification model".to_string(),
        ))
    }

    async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError> {
        // Note: This requires knowing the associated token account address
        // For now, return 0 - in production, derive the ATA
        warn!("Solana balance check requires ATA derivation - not implemented");
        Ok(0)
    }

    async fn transfer_to(
        &self,
        _destination: &str,
        _amount: u64,
    ) -> Result<TxResult, PaymentError> {
        // Signing and sending transactions requires proper key management
        warn!("Solana transfer_to not yet implemented - requires transaction signing");
        Err(PaymentError::UnsupportedChain(
            "Transfer requires transaction signing (not yet implemented)".to_string(),
        ))
    }

    async fn get_tx_status(&self, tx_hash: &str) -> Result<TxStatus, PaymentError> {
        let result = self.get_signature_statuses(&[tx_hash]).await?;

        match result.value.first() {
            Some(Some(status)) => {
                if status.err.is_some() {
                    Ok(TxStatus::Failed {
                        reason: format!("{:?}", status.err),
                    })
                } else if status.confirmation_status.as_deref() == Some("finalized") {
                    Ok(TxStatus::Confirmed {
                        confirmations: status.confirmations.unwrap_or(1),
                    })
                } else {
                    Ok(TxStatus::Pending)
                }
            }
            _ => Ok(TxStatus::Pending),
        }
    }

    async fn health_check(&self) -> Result<bool, PaymentError> {
        if !self.config.enabled {
            return Ok(false);
        }

        // Simple health check - get slot
        let params: Vec<()> = vec![];
        match self.rpc_call::<_, u64>("getSlot", params).await {
            Ok(slot) => {
                debug!("Solana RPC healthy, slot: {}", slot);
                Ok(true)
            }
            Err(e) => {
                warn!("Solana RPC health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_facilitator_creation() {
        let config = SolanaChainConfig {
            enabled: true,
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            usdc_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            operator_address: None,
        };

        let facilitator = SolanaFacilitator::new(config, "11111111111111111111111111111111".to_string())
            .await
            .unwrap();

        assert_eq!(facilitator.chain(), Chain::Solana);
        assert_eq!(facilitator.deposit_address(), "11111111111111111111111111111111");
    }
}

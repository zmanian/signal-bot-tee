//! NEAR Protocol chain facilitator.
//!
//! Uses NEAR RPC to verify USDC (NEP-141) transfers and manage deposits.

use super::{ChainFacilitator, PaymentPayload, PaymentVerification, TxResult};
use crate::config::NearChainConfig;
use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

/// NEAR chain facilitator.
///
/// Verifies USDC (NEP-141) transfers via NEAR JSON-RPC.
pub struct NearFacilitator {
    config: NearChainConfig,
    /// TEE-derived deposit account.
    deposit_account: String,
    /// HTTP client for RPC calls.
    client: reqwest::Client,
}

/// NEAR JSON-RPC request structure.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
    id: &'static str,
}

/// NEAR JSON-RPC response structure.
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

    /// Transaction status result from NEAR RPC.
    #[derive(Debug, Deserialize)]
    pub struct TxStatusResult {
        pub status: TxExecutionStatus,
        pub transaction: TransactionInfo,
        pub receipts_outcome: Vec<ReceiptOutcome>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    pub enum TxExecutionStatus {
        Success(SuccessStatus),
        Failure(FailureStatus),
    }

    #[derive(Debug, Deserialize)]
    pub struct SuccessStatus {
        #[serde(rename = "SuccessValue")]
        pub success_value: Option<String>,
        #[serde(rename = "SuccessReceiptId")]
        pub success_receipt_id: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct FailureStatus {
        #[serde(rename = "Failure")]
        pub failure: serde_json::Value,
    }

    #[derive(Debug, Deserialize)]
    pub struct TransactionInfo {
        pub signer_id: String,
        pub receiver_id: String,
        pub actions: Vec<ActionInfo>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "PascalCase")]
    pub enum ActionInfo {
        FunctionCall {
            method_name: String,
            args: String, // base64 encoded
        },
        #[serde(other)]
        Other,
    }

    #[derive(Debug, Deserialize)]
    pub struct ReceiptOutcome {
        pub id: String,
        pub outcome: ExecutionOutcome,
    }

    #[derive(Debug, Deserialize)]
    pub struct ExecutionOutcome {
        pub executor_id: String,
        pub status: TxExecutionStatus,
        pub logs: Vec<String>,
    }

    /// NEP-141 ft_transfer args.
    #[derive(Debug, Deserialize)]
    pub struct FtTransferArgs {
        pub receiver_id: String,
        pub amount: String,
        pub memo: Option<String>,
    }

    /// Query result for view functions.
    #[derive(Debug, Deserialize)]
    pub struct QueryResult {
        pub result: Vec<u8>,
        pub block_height: u64,
    }
}

use rpc_types::*;

impl NearFacilitator {
    /// Create a new NEAR facilitator.
    pub async fn new(
        config: NearChainConfig,
        deposit_account: String,
    ) -> Result<Self, PaymentError> {
        info!(
            "Initializing NEAR facilitator: rpc={}, usdc={}, deposit={}",
            config.rpc_url, config.usdc_contract, deposit_account
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PaymentError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            deposit_account,
            client,
        })
    }

    /// Derive deposit account from TEE key.
    ///
    /// NEAR uses implicit accounts (64-char hex of ed25519 pubkey).
    pub async fn derive_deposit_account(
        dstack: &dstack_client::DstackClient,
    ) -> Result<String, PaymentError> {
        // Derive 32-byte key from TEE
        let key_bytes = dstack
            .derive_key("x402-payments/near-deposit-wallet", None)
            .await
            .map_err(|e| PaymentError::Internal(format!("Failed to derive NEAR key: {}", e)))?;

        if key_bytes.len() < 32 {
            return Err(PaymentError::Internal(format!(
                "Derived key too short: {} bytes",
                key_bytes.len()
            )));
        }

        // Hash to get a deterministic 32-byte public key representation
        let mut hasher = Sha256::new();
        hasher.update(&key_bytes[..32]);
        let hash = hasher.finalize();

        // NEAR implicit account = 64 hex chars of the public key
        let account_id = hex::encode(hash);
        info!("Derived NEAR implicit account: {}", account_id);

        Ok(account_id)
    }

    /// Make a JSON-RPC call to NEAR.
    async fn rpc_call<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        method: &'static str,
        params: T,
    ) -> Result<R, PaymentError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method,
            params,
            id: "dontcare",
        };

        let response = self
            .client
            .post(&self.config.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| PaymentError::RpcError(format!("NEAR RPC request failed: {}", e)))?;

        let json_response: JsonRpcResponse<R> = response
            .json()
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to parse NEAR RPC response: {}", e)))?;

        if let Some(error) = json_response.error {
            return Err(PaymentError::RpcError(error.message));
        }

        json_response
            .result
            .ok_or_else(|| PaymentError::RpcError("Empty NEAR RPC response".to_string()))
    }

    /// Get transaction status.
    async fn get_tx_status_internal(&self, tx_hash: &str, sender_id: &str) -> Result<TxStatusResult, PaymentError> {
        let params = serde_json::json!({
            "tx_hash": tx_hash,
            "sender_account_id": sender_id,
            "wait_until": "EXECUTED"
        });

        self.rpc_call("tx", params).await
    }

    /// Query ft_balance_of for a NEP-141 token.
    async fn get_ft_balance(&self, token_contract: &str, account_id: &str) -> Result<u64, PaymentError> {
        let args = serde_json::json!({ "account_id": account_id });
        let args_base64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            serde_json::to_string(&args).unwrap(),
        );

        let params = serde_json::json!({
            "request_type": "call_function",
            "finality": "final",
            "account_id": token_contract,
            "method_name": "ft_balance_of",
            "args_base64": args_base64
        });

        let result: QueryResult = self.rpc_call("query", params).await?;

        // Result is JSON-encoded string of the balance
        let balance_str: String = serde_json::from_slice(&result.result)
            .map_err(|e| PaymentError::Internal(format!("Failed to parse balance: {}", e)))?;

        balance_str
            .parse::<u64>()
            .map_err(|e| PaymentError::Internal(format!("Invalid balance format: {}", e)))
    }

    /// Verify a USDC transfer transaction.
    async fn verify_usdc_transfer(
        &self,
        tx_hash: &str,
        expected_sender: Option<&str>,
        expected_amount: Option<u64>,
        expected_memo: &str,
    ) -> Result<PaymentVerification, PaymentError> {
        // We need the sender to query the transaction
        let sender = expected_sender.ok_or_else(|| {
            PaymentError::InvalidPayload("NEAR transfers require sender account ID".to_string())
        })?;

        // Get transaction status
        let tx_result = self.get_tx_status_internal(tx_hash, sender).await?;

        // Check if transaction succeeded
        match &tx_result.status {
            TxExecutionStatus::Failure(f) => {
                return Err(PaymentError::TxFailed(format!(
                    "Transaction failed: {:?}",
                    f.failure
                )));
            }
            TxExecutionStatus::Success(_) => {}
        }

        // Verify transaction was to USDC contract with ft_transfer
        if tx_result.transaction.receiver_id != self.config.usdc_contract {
            return Err(PaymentError::VerificationFailed(format!(
                "Transaction not to USDC contract: expected {}, got {}",
                self.config.usdc_contract, tx_result.transaction.receiver_id
            )));
        }

        // Find ft_transfer action
        let mut transfer_args: Option<FtTransferArgs> = None;
        for action in &tx_result.transaction.actions {
            if let ActionInfo::FunctionCall { method_name, args } = action {
                if method_name == "ft_transfer" {
                    // Decode base64 args
                    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, args)
                        .map_err(|e| PaymentError::Internal(format!("Failed to decode args: {}", e)))?;

                    transfer_args = Some(serde_json::from_slice(&decoded).map_err(|e| {
                        PaymentError::Internal(format!("Failed to parse ft_transfer args: {}", e))
                    })?);
                    break;
                }
            }
        }

        let args = transfer_args.ok_or_else(|| {
            PaymentError::NoTransferFound("No ft_transfer action found in transaction".to_string())
        })?;

        // Verify receiver is our deposit account
        if args.receiver_id != self.deposit_account {
            return Err(PaymentError::NoTransferFound(format!(
                "Transfer not to deposit account: expected {}, got {}",
                self.deposit_account, args.receiver_id
            )));
        }

        // Parse amount
        let amount: u64 = args
            .amount
            .parse()
            .map_err(|e| PaymentError::Internal(format!("Invalid amount: {}", e)))?;

        // Verify amount if expected
        if let Some(expected) = expected_amount {
            if amount != expected {
                return Err(PaymentError::AmountMismatch {
                    expected,
                    actual: amount,
                });
            }
        }

        // Verify memo matches user ID (phone number) if provided
        if !expected_memo.is_empty() {
            let memo = args.memo.as_deref().unwrap_or("");
            if memo != expected_memo {
                warn!(
                    "Memo mismatch: expected '{}', got '{}'",
                    expected_memo, memo
                );
                // Note: We warn but don't fail - memo is optional verification
            }
        }

        debug!(
            "Verified NEAR USDC transfer: from={}, amount={}, memo={:?}",
            sender, amount, args.memo
        );

        Ok(PaymentVerification {
            tx_hash: tx_hash.to_string(),
            amount_usdc: amount,
            from: Some(sender.to_string()),
            to: self.deposit_account.clone(),
            confirmations: 1, // NEAR finality is immediate
            verified: true,
        })
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
        // tx_hash contains the NEAR transaction hash
        // from contains the sender NEAR account ID (required for NEAR)
        // user_id contains the phone number (used as memo verification)

        self.verify_usdc_transfer(
            &payload.tx_hash,
            payload.from.as_deref(),
            payload.amount,
            &payload.user_id,
        )
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
        let balance = self
            .get_ft_balance(&self.config.usdc_contract, &self.deposit_account)
            .await?;

        debug!("NEAR deposit wallet balance: {} USDC (micro)", balance);
        Ok(balance)
    }

    async fn transfer_to(
        &self,
        _destination: &str,
        _amount: u64,
    ) -> Result<TxResult, PaymentError> {
        // Signing and sending transactions requires proper key management
        // This would need TEE-derived private key and transaction signing
        warn!("NEAR transfer_to not yet implemented - requires transaction signing");
        Err(PaymentError::UnsupportedChain(
            "Transfer requires transaction signing (not yet implemented)".to_string(),
        ))
    }

    async fn get_tx_status(&self, _tx_hash: &str) -> Result<TxStatus, PaymentError> {
        // For NEAR, we need the sender to query tx status
        // Since we don't have it here, return a simplified status
        // In practice, we should store tx sender mapping
        warn!("NEAR tx status requires sender account - returning pending");
        Ok(TxStatus::Pending)
    }

    async fn health_check(&self) -> Result<bool, PaymentError> {
        if !self.config.enabled {
            return Ok(false);
        }

        // Simple health check - query latest block
        let params = serde_json::json!({ "finality": "final" });
        match self.rpc_call::<_, serde_json::Value>("block", params).await {
            Ok(_) => {
                debug!("NEAR RPC healthy");
                Ok(true)
            }
            Err(e) => {
                warn!("NEAR RPC health check failed: {}", e);
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
        let config = NearChainConfig {
            enabled: true,
            rpc_url: "https://rpc.mainnet.near.org".to_string(),
            usdc_contract: "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1"
                .to_string(),
            operator_account: None,
        };

        let facilitator = NearFacilitator::new(config, "test.near".to_string())
            .await
            .unwrap();

        assert_eq!(facilitator.chain(), Chain::Near);
        assert_eq!(facilitator.deposit_address(), "test.near");
    }
}

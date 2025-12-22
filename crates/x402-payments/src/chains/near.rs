//! NEAR Protocol chain facilitator.
//!
//! Uses NEAR RPC to verify USDC (NEP-141) transfers and manage deposits.

use super::{ChainFacilitator, PaymentPayload, PaymentVerification, TxResult};
use crate::config::NearChainConfig;
use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// NEAR crypto for ed25519 signing
use near_crypto::{InMemorySigner, SecretKey};
// ed25519-dalek for direct keypair creation
use ed25519_dalek;
// NEAR primitives for transaction types
use near_primitives::{
    transaction::{Action, FunctionCallAction, SignedTransaction, Transaction, TransactionV0},
    types::{AccountId, BlockReference, Finality},
    views::AccessKeyView,
};
// NEAR JSON-RPC client
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;

/// NEAR chain facilitator.
///
/// Verifies USDC (NEP-141) transfers via NEAR JSON-RPC.
pub struct NearFacilitator {
    config: NearChainConfig,
    /// TEE-derived wallet signer
    signer: InMemorySigner,
    /// Deposit account ID (implicit account from public key)
    deposit_account: AccountId,
    /// JSON-RPC client
    rpc_client: JsonRpcClient,
    /// HTTP client for legacy RPC calls
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
    /// Create a new NEAR facilitator with TEE-derived wallet.
    pub async fn new(
        config: NearChainConfig,
        dstack: &dstack_client::DstackClient,
    ) -> Result<Self, PaymentError> {
        // Derive wallet from TEE
        let (signer, deposit_account) = Self::derive_wallet(dstack).await?;

        info!(
            "Initializing NEAR facilitator: rpc={}, usdc={}, deposit={}",
            config.rpc_url, config.usdc_contract, deposit_account
        );

        // Create JSON-RPC client
        let rpc_client = JsonRpcClient::connect(&config.rpc_url);

        // Create HTTP client for legacy RPC calls
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PaymentError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        let facilitator = Self {
            config,
            signer,
            deposit_account,
            rpc_client,
            client,
        };

        // Check if account is funded (warning only, don't fail construction)
        if let Err(e) = facilitator.ensure_account_funded().await {
            warn!(
                "NEAR deposit account may not be funded yet: {}. \
                Transfers will fail until the account is funded with at least 0.001 NEAR.",
                e
            );
        }

        Ok(facilitator)
    }

    /// Derive wallet (signer + account) from TEE key.
    ///
    /// NEAR uses implicit accounts (64-char hex of ed25519 pubkey).
    pub async fn derive_wallet(
        dstack: &dstack_client::DstackClient,
    ) -> Result<(InMemorySigner, AccountId), PaymentError> {
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

        // Use first 32 bytes as ed25519 secret key
        // Create the keypair directly from the 32-byte seed
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&key_bytes[..32]);

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let secret_key = SecretKey::ED25519(near_crypto::ED25519SecretKey(signing_key.to_keypair_bytes()));

        // Get public key
        let public_key = secret_key.public_key();

        // NEAR implicit account = 64 hex chars of the public key bytes
        let account_id_str = hex::encode(public_key.unwrap_as_ed25519().0);
        let account_id: AccountId = account_id_str
            .parse()
            .map_err(|e| PaymentError::Internal(format!("Invalid account ID: {}", e)))?;

        // Create signer
        let signer = InMemorySigner::from_secret_key(account_id.clone(), secret_key);

        info!("Derived NEAR implicit account: {}", account_id);

        Ok((signer, account_id))
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

    /// Get access key for transaction signing.
    async fn get_access_key(&self) -> Result<AccessKeyView, PaymentError> {
        let request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: near_primitives::views::QueryRequest::ViewAccessKey {
                account_id: self.deposit_account.clone(),
                public_key: self.signer.public_key(),
            },
        };

        let response = self
            .rpc_client
            .call(request)
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to get access key: {}", e)))?;

        match response.kind {
            QueryResponseKind::AccessKey(access_key) => Ok(access_key),
            _ => Err(PaymentError::RpcError(
                "Unexpected response type for access key query".to_string(),
            )),
        }
    }

    /// Get latest block for transaction.
    async fn get_latest_block(&self) -> Result<near_primitives::views::BlockView, PaymentError> {
        let request = methods::block::RpcBlockRequest {
            block_reference: BlockReference::Finality(Finality::Final),
        };

        let response = self
            .rpc_client
            .call(request)
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to get latest block: {}", e)))?;

        Ok(response)
    }

    /// Check if the deposit account is funded.
    ///
    /// Implicit accounts on NEAR must be funded before they can perform transactions.
    /// This method queries the account and checks if it has sufficient NEAR balance.
    async fn ensure_account_funded(&self) -> Result<(), PaymentError> {
        let request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: near_primitives::views::QueryRequest::ViewAccount {
                account_id: self.deposit_account.clone(),
            },
        };

        match self.rpc_client.call(request).await {
            Ok(response) => {
                match response.kind {
                    QueryResponseKind::ViewAccount(account_view) => {
                        // Check if account has at least 0.001 NEAR for gas (1e21 yoctoNEAR)
                        const MIN_BALANCE: u128 = 1_000_000_000_000_000_000_000; // 0.001 NEAR

                        if account_view.amount < MIN_BALANCE {
                            return Err(PaymentError::Internal(format!(
                                "Deposit account {} has insufficient NEAR balance: {} yoctoNEAR (need at least {} for gas)",
                                self.deposit_account, account_view.amount, MIN_BALANCE
                            )));
                        }

                        debug!(
                            "Deposit account {} is funded with {} yoctoNEAR",
                            self.deposit_account, account_view.amount
                        );
                        Ok(())
                    }
                    _ => Err(PaymentError::RpcError(
                        "Unexpected response type for account query".to_string(),
                    )),
                }
            }
            Err(e) => {
                // Account doesn't exist
                Err(PaymentError::Internal(format!(
                    "Deposit account {} does not exist. Please fund the implicit account with at least 0.001 NEAR before use. Error: {}",
                    self.deposit_account, e
                )))
            }
        }
    }

    /// Broadcast signed transaction and wait for finality.
    async fn broadcast_tx_commit(&self, signed_tx: SignedTransaction) -> Result<TxResult, PaymentError> {
        let request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction: signed_tx,
        };

        let response = self
            .rpc_client
            .call(request)
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to broadcast transaction: {}", e)))?;

        // Check if transaction succeeded
        let success = match response.status {
            near_primitives::views::FinalExecutionStatus::SuccessValue(_) => true,
            near_primitives::views::FinalExecutionStatus::Failure(err) => {
                return Err(PaymentError::TxFailed(format!("Transaction failed: {:?}", err)));
            }
            _ => false,
        };

        Ok(TxResult {
            tx_hash: response.transaction.hash.to_string(),
            block_number: None, // NEAR uses block hash, not number in this response
            success,
        })
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
            to: self.deposit_account.to_string(),
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
        self.deposit_account.to_string()
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
            .get_ft_balance(&self.config.usdc_contract, &self.deposit_account.to_string())
            .await?;

        debug!("NEAR deposit wallet balance: {} USDC (micro)", balance);
        Ok(balance)
    }

    async fn transfer_to(
        &self,
        destination: &str,
        amount: u64,
    ) -> Result<TxResult, PaymentError> {
        info!(
            "Transferring {} USDC from {} to {} on NEAR",
            amount, self.deposit_account, destination
        );

        // Ensure the deposit account is funded before attempting transfer
        self.ensure_account_funded().await?;

        // Parse destination as AccountId
        let receiver_id: AccountId = destination
            .parse()
            .map_err(|e| PaymentError::Internal(format!("Invalid destination account: {}", e)))?;

        // Get current nonce and block hash
        let access_key = self.get_access_key().await?;
        let block = self.get_latest_block().await?;

        // Build ft_transfer args
        let ft_transfer_args = serde_json::json!({
            "receiver_id": receiver_id.to_string(),
            "amount": amount.to_string(),
            "memo": format!("Sweep to operator {}", destination),
        });

        let args_json = serde_json::to_string(&ft_transfer_args)
            .map_err(|e| PaymentError::Internal(format!("Failed to serialize args: {}", e)))?;

        // Parse USDC contract as AccountId
        let usdc_contract: AccountId = self.config.usdc_contract
            .parse()
            .map_err(|e| PaymentError::Internal(format!("Invalid USDC contract: {}", e)))?;

        // Create ft_transfer action
        let action = Action::FunctionCall(Box::new(FunctionCallAction {
            method_name: "ft_transfer".to_string(),
            args: args_json.into_bytes(),
            gas: 30_000_000_000_000, // 30 TGas
            deposit: 1, // 1 yoctoNEAR (required for NEP-141)
        }));

        // Build transaction V0
        let transaction_v0 = TransactionV0 {
            signer_id: self.deposit_account.clone(),
            public_key: self.signer.public_key(),
            nonce: access_key.nonce + 1,
            receiver_id: usdc_contract.clone(),
            block_hash: block.header.hash,
            actions: vec![action],
        };

        let transaction = Transaction::V0(transaction_v0);

        // Sign transaction
        let signed_tx = SignedTransaction::new(
            self.signer.sign(transaction.get_hash_and_size().0.as_ref()),
            transaction,
        );

        // Broadcast via broadcast_tx_commit (waits for finality)
        let tx_result = self.broadcast_tx_commit(signed_tx).await?;

        info!(
            "NEAR transfer broadcasted: tx_hash={}, success={}",
            tx_result.tx_hash, tx_result.success
        );

        Ok(tx_result)
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
    // Note: Integration tests for NearFacilitator require a real DstackClient
    // to derive the ed25519 wallet keypair from TEE-derived entropy.
    //
    // Unit tests here would need to mock DstackClient, which is complex.
    // Instead, we rely on:
    // 1. Integration tests that run in a real TEE environment (marked #[ignore])
    // 2. The comprehensive tests in credits::store that validate the overall flow
    // 3. Manual testing with real NEAR testnet/mainnet transactions
    //
    // Future work: Create a mock DstackClient for unit testing chain facilitators
}

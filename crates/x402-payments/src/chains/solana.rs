//! Solana chain facilitator with wallet support.
//!
//! Verifies SPL token (USDC) transfers and supports sweeping to operator.

use super::{ChainFacilitator, PaymentPayload, PaymentVerification, TxResult};
use crate::config::SolanaChainConfig;
use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

// Solana SDK imports
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    signer::SeedDerivable,
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::instruction::transfer_checked;

/// Solana chain facilitator.
///
/// Uses Solana SDK for SPL token payment verification and wallet operations.
pub struct SolanaFacilitator {
    config: SolanaChainConfig,
    /// TEE-derived deposit wallet keypair.
    wallet_keypair: Keypair,
    /// Deposit wallet public key.
    wallet_pubkey: Pubkey,
    /// Solana RPC client.
    rpc_client: RpcClient,
    /// HTTP client for raw JSON-RPC calls.
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
        dstack: &dstack_client::DstackClient,
    ) -> Result<Self, PaymentError> {
        // Derive wallet keypair
        let (wallet_keypair, wallet_pubkey) = Self::derive_wallet(dstack).await?;

        info!(
            "Initializing Solana facilitator: rpc={}, usdc_mint={}, deposit={}",
            config.rpc_url, config.usdc_mint, wallet_pubkey
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PaymentError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        // Create RPC client with confirmed commitment
        let rpc_client = RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );

        Ok(Self {
            config,
            wallet_keypair,
            wallet_pubkey,
            rpc_client,
            client,
        })
    }

    /// Derive deposit wallet from TEE key.
    ///
    /// Returns (Keypair, Pubkey) for the TEE-derived wallet.
    pub async fn derive_wallet(
        dstack: &dstack_client::DstackClient,
    ) -> Result<(Keypair, Pubkey), PaymentError> {
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

        // Hash to get a deterministic 32-byte seed for keypair
        let mut hasher = Sha256::new();
        hasher.update(&key_bytes[..32]);
        let seed = hasher.finalize();

        // Create keypair from 32-byte seed using SeedDerivable trait
        let keypair = Keypair::from_seed(seed.as_slice())
            .map_err(|e| PaymentError::Internal(format!("Failed to create Solana keypair: {}", e)))?;

        let pubkey = keypair.pubkey();
        info!("Derived Solana deposit wallet: {}", pubkey);

        Ok((keypair, pubkey))
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
            if owner != self.wallet_pubkey.to_string() {
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
                self.wallet_pubkey, signature
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
            to: self.wallet_pubkey.to_string(),
            confirmations: 1, // Solana finality is fast
            verified: true,
        })
    }

    /// Parse a Pubkey from string.
    fn parse_pubkey(s: &str) -> Result<Pubkey, PaymentError> {
        s.parse()
            .map_err(|e| PaymentError::Internal(format!("Invalid Solana pubkey '{}': {}", s, e)))
    }
}

#[async_trait]
impl ChainFacilitator for SolanaFacilitator {
    fn chain(&self) -> Chain {
        Chain::Solana
    }

    fn deposit_address(&self) -> String {
        self.wallet_pubkey.to_string()
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
        // Parse USDC mint
        let usdc_mint = Self::parse_pubkey(&self.config.usdc_mint)?;

        // Derive the associated token account (ATA) for our wallet
        let ata = get_associated_token_address(&self.wallet_pubkey, &usdc_mint);

        debug!(
            "Getting USDC balance for wallet {} at ATA {}",
            self.wallet_pubkey, ata
        );

        // Get token account balance
        match self.rpc_client.get_token_account_balance(&ata) {
            Ok(balance) => {
                let amount = balance
                    .amount
                    .parse::<u64>()
                    .map_err(|e| PaymentError::Internal(format!("Invalid balance amount: {}", e)))?;

                debug!("Solana wallet balance: {} USDC (raw amount)", amount);
                Ok(amount)
            }
            Err(e) => {
                // If account doesn't exist, balance is 0
                if e.to_string().contains("could not find account") {
                    debug!("ATA {} not found, balance is 0", ata);
                    Ok(0)
                } else {
                    Err(PaymentError::RpcError(format!(
                        "Failed to get Solana token balance: {}",
                        e
                    )))
                }
            }
        }
    }

    async fn transfer_to(
        &self,
        destination: &str,
        amount: u64,
    ) -> Result<TxResult, PaymentError> {
        info!(
            "Transferring {} USDC from {} to {}",
            amount, self.wallet_pubkey, destination
        );

        // Parse addresses
        let usdc_mint = Self::parse_pubkey(&self.config.usdc_mint)?;
        let destination_pubkey = Self::parse_pubkey(destination)?;

        // Derive ATAs
        let source_ata = get_associated_token_address(&self.wallet_pubkey, &usdc_mint);
        let dest_ata = get_associated_token_address(&destination_pubkey, &usdc_mint);

        debug!(
            "Transfer from ATA {} to ATA {}",
            source_ata, dest_ata
        );

        // Check if destination ATA exists, create if not
        let mut instructions = Vec::new();
        match self.rpc_client.get_account(&dest_ata) {
            Err(_) => {
                // ATA doesn't exist, create it
                debug!("Destination ATA {} does not exist, creating it", dest_ata);
                let create_ata_ix = create_associated_token_account(
                    &self.wallet_pubkey,  // payer
                    &destination_pubkey,  // wallet owner
                    &usdc_mint,           // mint
                    &spl_token::id(),     // token program
                );
                instructions.push(create_ata_ix);
            }
            Ok(_) => {
                debug!("Destination ATA {} already exists", dest_ata);
            }
        }

        // Create transfer_checked instruction (USDC has 6 decimals)
        let transfer_ix = transfer_checked(
            &spl_token::id(),
            &source_ata,
            &usdc_mint,
            &dest_ata,
            &self.wallet_pubkey,
            &[],
            amount,
            6, // USDC decimals
        )
        .map_err(|e| PaymentError::Internal(format!("Failed to create transfer instruction: {}", e)))?;
        instructions.push(transfer_ix);

        // Get recent blockhash
        let recent_blockhash = self
            .rpc_client
            .get_latest_blockhash()
            .map_err(|e| PaymentError::RpcError(format!("Failed to get recent blockhash: {}", e)))?;

        // Create and sign transaction
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.wallet_pubkey),
            &[&self.wallet_keypair],
            recent_blockhash,
        );

        // Send transaction
        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| PaymentError::TxFailed(format!("Transfer failed: {}", e)))?;

        info!(
            "Successfully transferred {} USDC to {}, signature: {}",
            amount, destination, signature
        );

        Ok(TxResult {
            tx_hash: signature.to_string(),
            block_number: None, // Solana uses slots, not block numbers
            success: true,
        })
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

    // Note: Tests requiring DstackClient are integration tests that run in TEE
    // Unit tests here validate parsing and type conversions

    #[test]
    fn test_parse_pubkey() {
        // Valid Solana pubkey
        let result = SolanaFacilitator::parse_pubkey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert!(result.is_ok());

        // Invalid pubkey
        let result = SolanaFacilitator::parse_pubkey("invalid");
        assert!(result.is_err());
    }
}

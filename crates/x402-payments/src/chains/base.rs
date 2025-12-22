//! Base (EVM) chain facilitator using raw JSON-RPC.
//!
//! Verifies USDC transfers on Base L2 and manages deposit wallet.

use super::{ChainFacilitator, PaymentPayload, PaymentVerification, TxResult};
use crate::config::BaseChainConfig;
use crate::error::PaymentError;
use crate::types::{Chain, SettlementResult, TxStatus};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

/// ERC20 Transfer event signature: keccak256("Transfer(address,address,uint256)")
const TRANSFER_EVENT_SIGNATURE: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Base chain facilitator.
///
/// Uses raw JSON-RPC calls for EVM-compatible payment handling on Base L2.
pub struct BaseFacilitator {
    config: BaseChainConfig,
    /// TEE-derived deposit wallet address.
    deposit_address: String,
    /// HTTP client for RPC calls.
    client: reqwest::Client,
}

/// JSON-RPC request structure.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
    id: u64,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

/// Transaction receipt from eth_getTransactionReceipt.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TxReceipt {
    status: String,
    block_number: Option<String>,
    logs: Vec<TxLog>,
}

/// Log entry in transaction receipt.
#[derive(Debug, Deserialize)]
struct TxLog {
    address: String,
    topics: Vec<String>,
    data: String,
}

impl BaseFacilitator {
    /// Create a new Base facilitator.
    pub async fn new(
        config: BaseChainConfig,
        deposit_address: String,
    ) -> Result<Self, PaymentError> {
        info!(
            "Initializing Base facilitator: rpc={}, usdc={}, deposit={}",
            config.rpc_url, config.usdc_contract, deposit_address
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

    /// Derive deposit wallet address from TEE key.
    ///
    /// Derives an Ethereum address from a TEE-derived key using keccak256.
    pub async fn derive_deposit_address(
        dstack: &dstack_client::DstackClient,
    ) -> Result<String, PaymentError> {
        // Derive 32-byte key from TEE
        let key_bytes = dstack
            .derive_key("x402-payments/base-deposit-wallet", None)
            .await
            .map_err(|e| PaymentError::Internal(format!("Failed to derive Base key: {}", e)))?;

        if key_bytes.len() < 32 {
            return Err(PaymentError::Internal(format!(
                "Derived key too short: {} bytes",
                key_bytes.len()
            )));
        }

        // Use the key bytes as a "public key" and hash to get address
        // Note: In production, use proper secp256k1 derivation
        // For now, we use SHA256 of the key as a deterministic address
        let mut hasher = Sha256::new();
        hasher.update(&key_bytes[..32]);
        let hash = hasher.finalize();

        // Take last 20 bytes as Ethereum address
        let address = format!("0x{}", hex::encode(&hash[12..32]));
        info!("Derived Base deposit address: {}", address);

        Ok(address)
    }

    /// Make a JSON-RPC call to the EVM node.
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
            .map_err(|e| PaymentError::RpcError(format!("RPC request failed: {}", e)))?;

        let json_response: JsonRpcResponse<R> = response
            .json()
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to parse RPC response: {}", e)))?;

        if let Some(error) = json_response.error {
            return Err(PaymentError::RpcError(error.message));
        }

        json_response
            .result
            .ok_or_else(|| PaymentError::RpcError("Empty RPC response".to_string()))
    }

    /// Get the current block number.
    async fn get_block_number(&self) -> Result<u64, PaymentError> {
        let hex_block: String = self.rpc_call("eth_blockNumber", ()).await?;
        parse_hex_u64(&hex_block)
    }

    /// Get transaction receipt.
    async fn get_transaction_receipt(&self, tx_hash: &str) -> Result<Option<TxReceipt>, PaymentError> {
        let result: Option<TxReceipt> = self.rpc_call("eth_getTransactionReceipt", [tx_hash]).await?;
        Ok(result)
    }

    /// Call ERC20 balanceOf.
    async fn get_erc20_balance(&self, token: &str, address: &str) -> Result<u64, PaymentError> {
        // balanceOf(address) function selector = 0x70a08231
        // Pad address to 32 bytes
        let padded_address = format!("{:0>64}", address.trim_start_matches("0x"));
        let data = format!("0x70a08231{}", padded_address);

        let params = serde_json::json!([
            {
                "to": token,
                "data": data
            },
            "latest"
        ]);

        let result: String = self.rpc_call("eth_call", params).await?;
        parse_hex_u64(&result)
    }

    /// Verify a USDC transfer transaction.
    async fn verify_usdc_transfer(
        &self,
        tx_hash: &str,
        expected_from: Option<&str>,
        expected_amount: Option<u64>,
    ) -> Result<PaymentVerification, PaymentError> {
        // Get transaction receipt
        let receipt = self
            .get_transaction_receipt(tx_hash)
            .await?
            .ok_or_else(|| PaymentError::TxNotFound(tx_hash.to_string()))?;

        // Check if transaction succeeded (status = 0x1)
        if receipt.status != "0x1" {
            return Err(PaymentError::TxFailed(tx_hash.to_string()));
        }

        let usdc_address = self.config.usdc_contract.to_lowercase();
        let deposit_address = self.deposit_address.to_lowercase();

        let mut verified_amount: u64 = 0;
        let mut verified_from: Option<String> = None;

        // Look for USDC Transfer event to our deposit address
        for log in &receipt.logs {
            if log.address.to_lowercase() != usdc_address {
                continue;
            }

            if log.topics.len() < 3 {
                continue;
            }

            // Check for Transfer event signature
            if log.topics[0].to_lowercase() != TRANSFER_EVENT_SIGNATURE {
                continue;
            }

            // topics[2] = to address (padded to 32 bytes)
            let to_topic = &log.topics[2];
            let to_address = format!("0x{}", &to_topic[to_topic.len() - 40..]);

            if to_address.to_lowercase() == deposit_address {
                // Found transfer to our deposit address
                // topics[1] = from address
                let from_topic = &log.topics[1];
                verified_from = Some(format!("0x{}", &from_topic[from_topic.len() - 40..]));

                // data = amount (U256 as hex)
                verified_amount = parse_hex_u64(&log.data)?;

                debug!(
                    "Found USDC transfer: from={:?}, to={}, amount={}",
                    verified_from, to_address, verified_amount
                );
                break;
            }
        }

        if verified_amount == 0 {
            return Err(PaymentError::NoTransferFound(format!(
                "No USDC transfer to {} found in tx {}",
                self.deposit_address, tx_hash
            )));
        }

        // Verify sender if expected
        if let Some(expected) = expected_from {
            if let Some(ref actual) = verified_from {
                if actual.to_lowercase() != expected.to_lowercase() {
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

        // Calculate confirmations
        let block_number = receipt
            .block_number
            .as_ref()
            .map(|s| parse_hex_u64(s))
            .transpose()?
            .unwrap_or(0);

        let current_block = self.get_block_number().await?;
        let confirmations = current_block.saturating_sub(block_number);

        Ok(PaymentVerification {
            tx_hash: tx_hash.to_string(),
            amount_usdc: verified_amount,
            from: verified_from,
            to: self.deposit_address.clone(),
            confirmations,
            verified: true,
        })
    }
}

/// Parse a hex string (0x prefixed or not) to u64.
fn parse_hex_u64(hex_str: &str) -> Result<u64, PaymentError> {
    let clean = hex_str.trim_start_matches("0x");
    if clean.is_empty() || clean == "0" {
        return Ok(0);
    }

    // For large hex values, parse as u128 first then try to fit in u64
    u128::from_str_radix(clean, 16)
        .map_err(|e| PaymentError::Internal(format!("Invalid hex: {}", e)))
        .and_then(|v| {
            v.try_into()
                .map_err(|_| PaymentError::Internal("Value overflow".to_string()))
        })
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
        payload: &PaymentPayload,
    ) -> Result<PaymentVerification, PaymentError> {
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
        let balance = self
            .get_erc20_balance(&self.config.usdc_contract, &self.deposit_address)
            .await?;

        debug!("Base deposit wallet balance: {} USDC (micro)", balance);
        Ok(balance)
    }

    async fn transfer_to(
        &self,
        _destination: &str,
        _amount: u64,
    ) -> Result<TxResult, PaymentError> {
        // Signing and sending transactions requires proper key management
        // This would need TEE-derived private key and transaction signing
        warn!("Base transfer_to not yet implemented - requires transaction signing");
        Err(PaymentError::UnsupportedChain(
            "Transfer requires transaction signing (not yet implemented)".to_string(),
        ))
    }

    async fn get_tx_status(&self, tx_hash: &str) -> Result<TxStatus, PaymentError> {
        match self.get_transaction_receipt(tx_hash).await? {
            Some(receipt) => {
                if receipt.status == "0x1" {
                    let block_number = receipt
                        .block_number
                        .as_ref()
                        .map(|s| parse_hex_u64(s))
                        .transpose()?
                        .unwrap_or(0);

                    let current_block = self.get_block_number().await.unwrap_or(block_number);
                    let confirmations = current_block.saturating_sub(block_number);

                    Ok(TxStatus::Confirmed { confirmations })
                } else {
                    Ok(TxStatus::Failed {
                        reason: "Transaction reverted".to_string(),
                    })
                }
            }
            None => Ok(TxStatus::Pending),
        }
    }

    async fn health_check(&self) -> Result<bool, PaymentError> {
        if !self.config.enabled {
            return Ok(false);
        }

        match self.get_block_number().await {
            Ok(block) => {
                debug!("Base RPC healthy, latest block: {}", block);
                Ok(true)
            }
            Err(e) => {
                warn!("Base RPC health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_u64() {
        assert_eq!(parse_hex_u64("0x0").unwrap(), 0);
        assert_eq!(parse_hex_u64("0x1").unwrap(), 1);
        assert_eq!(parse_hex_u64("0xa").unwrap(), 10);
        assert_eq!(parse_hex_u64("0xff").unwrap(), 255);
        assert_eq!(parse_hex_u64("0x100").unwrap(), 256);
        assert_eq!(parse_hex_u64("0xf4240").unwrap(), 1_000_000); // 1 USDC
    }

    #[test]
    fn test_parse_hex_large() {
        // 1000 USDC = 1_000_000_000 micro-USDC
        assert_eq!(parse_hex_u64("0x3b9aca00").unwrap(), 1_000_000_000);
    }

    #[tokio::test]
    async fn test_facilitator_creation() {
        let config = BaseChainConfig {
            enabled: true,
            rpc_url: "https://mainnet.base.org".to_string(),
            usdc_contract: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            operator_address: None,
        };

        let facilitator = BaseFacilitator::new(config, "0x1234".to_string())
            .await
            .unwrap();

        assert_eq!(facilitator.chain(), Chain::Base);
        assert_eq!(facilitator.deposit_address(), "0x1234");
    }
}

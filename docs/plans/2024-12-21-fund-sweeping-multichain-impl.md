# Fund Sweeping & Multi-Chain Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable automatic fund sweeping from TEE deposit wallets to operator wallets across Base, NEAR, and Solana chains.

**Architecture:** Each chain facilitator derives a proper keypair from TEE entropy, stores it securely, and uses chain-specific SDKs (alloy, near-*, solana-sdk) to sign and broadcast transfer transactions.

**Tech Stack:** Rust, alloy (EVM), near-crypto/near-jsonrpc-client (NEAR), solana-sdk/spl-token (Solana), dstack-client (TEE key derivation)

---

## Task 1: Add Dependencies

**Files:**
- Modify: `crates/x402-payments/Cargo.toml`

**Step 1: Add alloy dependency for Base/EVM**

Add to `[dependencies]` section:

```toml
# EVM (Base) - transaction signing and provider
alloy = { version = "0.9", default-features = false, features = [
    "providers",
    "provider-http",
    "signers",
    "signer-local",
    "network",
    "contract",
    "sol-types",
    "consensus",
] }
```

**Step 2: Add NEAR dependencies**

```toml
# NEAR Protocol - transaction signing
near-crypto = "0.27"
near-primitives = "0.27"
near-jsonrpc-client = "0.13"
near-jsonrpc-primitives = "0.27"
```

**Step 3: Add Solana dependencies**

```toml
# Solana - transaction signing
solana-sdk = "2.1"
solana-client = "2.1"
spl-token = "7"
spl-associated-token-account = "5"
```

**Step 4: Verify dependencies compile**

Run: `cargo check -p x402-payments`
Expected: Compiles successfully (may take a while to download)

**Step 5: Commit**

```bash
git add crates/x402-payments/Cargo.toml
git commit -m "deps: add alloy, near-*, and solana-sdk for transaction signing"
```

---

## Task 2: Base Chain - Refactor to Use Alloy Wallet

**Files:**
- Modify: `crates/x402-payments/src/chains/base.rs`

**Step 1: Add alloy imports and update struct**

Replace the existing imports and struct definition. At the top of the file, add:

```rust
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
```

Update the struct to store the wallet and provider:

```rust
pub struct BaseFacilitator {
    config: BaseChainConfig,
    /// TEE-derived deposit wallet address (checksummed).
    deposit_address: Address,
    /// Signer for transaction signing.
    signer: PrivateKeySigner,
    /// HTTP client for RPC calls (keep for verification).
    client: reqwest::Client,
}
```

**Step 2: Update derive_deposit_address to return proper keypair**

Replace the `derive_deposit_address` function:

```rust
/// Derive deposit wallet keypair from TEE key.
///
/// Returns (signer, address) tuple.
pub async fn derive_wallet(
    dstack: &dstack_client::DstackClient,
) -> Result<(PrivateKeySigner, Address), PaymentError> {
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

    // Create signer from the 32-byte key
    let key_array: [u8; 32] = key_bytes[..32]
        .try_into()
        .map_err(|_| PaymentError::Internal("Key conversion failed".to_string()))?;

    let signer = PrivateKeySigner::from_bytes(&key_array.into())
        .map_err(|e| PaymentError::Internal(format!("Invalid private key: {}", e)))?;

    let address = signer.address();
    info!("Derived Base deposit address: {}", address);

    Ok((signer, address))
}
```

**Step 3: Update constructor**

```rust
/// Create a new Base facilitator with TEE-derived wallet.
pub async fn new(
    config: BaseChainConfig,
    dstack: &dstack_client::DstackClient,
) -> Result<Self, PaymentError> {
    let (signer, deposit_address) = Self::derive_wallet(dstack).await?;

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
        signer,
        client,
    })
}
```

**Step 4: Update deposit_address method**

```rust
fn deposit_address(&self) -> String {
    self.deposit_address.to_string()
}
```

**Step 5: Implement transfer_to**

Define the ERC20 transfer interface and implement the transfer:

```rust
// Define ERC20 transfer interface using alloy sol! macro
sol! {
    function transfer(address to, uint256 value) external returns (bool);
}

impl BaseFacilitator {
    // ... existing methods ...

    /// Transfer USDC from deposit wallet to destination.
    async fn execute_transfer(
        &self,
        destination: &str,
        amount: u64,
    ) -> Result<TxResult, PaymentError> {
        // Parse destination address
        let to_address: Address = destination
            .parse()
            .map_err(|e| PaymentError::Internal(format!("Invalid destination address: {}", e)))?;

        // Parse USDC contract address
        let usdc_address: Address = self.config.usdc_contract
            .parse()
            .map_err(|e| PaymentError::Internal(format!("Invalid USDC contract: {}", e)))?;

        // Build the provider with signer
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .on_http(self.config.rpc_url.parse().map_err(|e| {
                PaymentError::Internal(format!("Invalid RPC URL: {}", e))
            })?);

        // Encode the transfer call
        let call = transferCall {
            to: to_address,
            value: U256::from(amount),
        };

        // Build and send transaction
        let tx = alloy::rpc::types::TransactionRequest::default()
            .to(usdc_address)
            .input(call.abi_encode().into());

        info!(
            "Sending Base USDC transfer: {} -> {}, amount: {}",
            self.deposit_address, to_address, amount
        );

        let pending = provider
            .send_transaction(tx)
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to send transaction: {}", e)))?;

        let tx_hash = pending.tx_hash().to_string();
        info!("Base transfer submitted: {}", tx_hash);

        // Wait for receipt
        let receipt = pending
            .get_receipt()
            .await
            .map_err(|e| PaymentError::RpcError(format!("Failed to get receipt: {}", e)))?;

        let success = receipt.status();
        let block_number = receipt.block_number;

        Ok(TxResult {
            tx_hash,
            block_number,
            success,
        })
    }
}
```

**Step 6: Update the ChainFacilitator impl for transfer_to**

Replace the existing `transfer_to` implementation:

```rust
async fn transfer_to(
    &self,
    destination: &str,
    amount: u64,
) -> Result<TxResult, PaymentError> {
    self.execute_transfer(destination, amount).await
}
```

**Step 7: Verify it compiles**

Run: `cargo check -p x402-payments`
Expected: Compiles successfully

**Step 8: Commit**

```bash
git add crates/x402-payments/src/chains/base.rs
git commit -m "feat(base): implement transfer_to with alloy wallet signing"
```

---

## Task 3: NEAR Chain - Implement Transaction Signing

**Files:**
- Modify: `crates/x402-payments/src/chains/near.rs`

**Step 1: Add near-crypto imports and update struct**

Add imports at the top:

```rust
use near_crypto::{InMemorySigner, KeyType, SecretKey, Signer};
use near_primitives::account::AccessKey;
use near_primitives::action::{Action, FunctionCallAction};
use near_primitives::hash::CryptoHash;
use near_primitives::transaction::{SignedTransaction, Transaction};
use near_primitives::types::{AccountId, BlockReference, Finality, Gas, Nonce};
use near_jsonrpc_client::{methods, JsonRpcClient as NearJsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
```

Update the struct:

```rust
pub struct NearFacilitator {
    config: NearChainConfig,
    /// TEE-derived deposit account ID (implicit account).
    deposit_account: AccountId,
    /// In-memory signer for transaction signing.
    signer: InMemorySigner,
    /// NEAR JSON-RPC client.
    near_client: NearJsonRpcClient,
    /// HTTP client for legacy RPC calls.
    client: reqwest::Client,
}
```

**Step 2: Update derive_deposit_account to return signer**

```rust
/// Derive deposit wallet keypair from TEE key.
///
/// Returns (signer, account_id) tuple. Uses implicit account (ed25519 pubkey as hex).
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

    // Create ed25519 secret key from bytes
    let secret_key = SecretKey::from_seed(KeyType::ED25519, &hex::encode(&key_bytes[..32]));
    let public_key = secret_key.public_key();

    // Implicit account ID is the hex-encoded public key (without ed25519: prefix)
    let public_key_hex = match &public_key {
        near_crypto::PublicKey::ED25519(key) => hex::encode(key.as_ref()),
        _ => return Err(PaymentError::Internal("Unexpected key type".to_string())),
    };

    let account_id: AccountId = public_key_hex
        .parse()
        .map_err(|e| PaymentError::Internal(format!("Invalid account ID: {}", e)))?;

    let signer = InMemorySigner::from_secret_key(account_id.clone(), secret_key);

    info!("Derived NEAR implicit account: {}", account_id);

    Ok((signer, account_id))
}
```

**Step 3: Update constructor**

```rust
/// Create a new NEAR facilitator with TEE-derived wallet.
pub async fn new(
    config: NearChainConfig,
    dstack: &dstack_client::DstackClient,
) -> Result<Self, PaymentError> {
    let (signer, deposit_account) = Self::derive_wallet(dstack).await?;

    info!(
        "Initializing NEAR facilitator: rpc={}, usdc={}, deposit={}",
        config.rpc_url, config.usdc_contract, deposit_account
    );

    let near_client = NearJsonRpcClient::connect(&config.rpc_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| PaymentError::Internal(format!("Failed to create HTTP client: {}", e)))?;

    Ok(Self {
        config,
        deposit_account,
        signer,
        near_client,
        client,
    })
}
```

**Step 4: Implement transfer_to**

```rust
/// Transfer USDC (NEP-141) from deposit wallet to destination.
async fn execute_transfer(
    &self,
    destination: &str,
    amount: u64,
) -> Result<TxResult, PaymentError> {
    let receiver_id: AccountId = destination
        .parse()
        .map_err(|e| PaymentError::Internal(format!("Invalid destination account: {}", e)))?;

    let usdc_contract: AccountId = self.config.usdc_contract
        .parse()
        .map_err(|e| PaymentError::Internal(format!("Invalid USDC contract: {}", e)))?;

    // Get current nonce and block hash
    let access_key_query = methods::query::RpcQueryRequest {
        block_reference: BlockReference::Finality(Finality::Final),
        request: near_primitives::views::QueryRequest::ViewAccessKey {
            account_id: self.deposit_account.clone(),
            public_key: self.signer.public_key(),
        },
    };

    let access_key_response = self.near_client
        .call(access_key_query)
        .await
        .map_err(|e| PaymentError::RpcError(format!("Failed to get access key: {}", e)))?;

    let (nonce, block_hash) = match access_key_response.kind {
        QueryResponseKind::AccessKey(access_key) => {
            (access_key.nonce + 1, access_key_response.block_hash)
        }
        _ => return Err(PaymentError::Internal("Unexpected query response".to_string())),
    };

    // Build ft_transfer action
    let args = serde_json::json!({
        "receiver_id": receiver_id.to_string(),
        "amount": amount.to_string(),
    });

    let action = Action::FunctionCall(Box::new(FunctionCallAction {
        method_name: "ft_transfer".to_string(),
        args: args.to_string().into_bytes(),
        gas: 30_000_000_000_000, // 30 TGas
        deposit: 1, // 1 yoctoNEAR required for ft_transfer
    }));

    // Build transaction
    let transaction = Transaction {
        signer_id: self.deposit_account.clone(),
        public_key: self.signer.public_key(),
        nonce,
        receiver_id: usdc_contract,
        block_hash,
        actions: vec![action],
    };

    // Sign transaction
    let signature = self.signer.sign(transaction.get_hash_and_size().0.as_ref());
    let signed_tx = SignedTransaction::new(signature, transaction);

    info!(
        "Sending NEAR ft_transfer: {} -> {}, amount: {}",
        self.deposit_account, receiver_id, amount
    );

    // Broadcast transaction
    let request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
        signed_transaction: signed_tx,
    };

    let response = self.near_client
        .call(request)
        .await
        .map_err(|e| PaymentError::RpcError(format!("Failed to broadcast tx: {}", e)))?;

    let tx_hash = response.transaction.hash.to_string();
    let success = response.status.is_success();

    info!("NEAR transfer complete: {}, success: {}", tx_hash, success);

    Ok(TxResult {
        tx_hash,
        block_number: None, // NEAR doesn't use block numbers the same way
        success,
    })
}
```

**Step 5: Update ChainFacilitator impl**

```rust
async fn transfer_to(
    &self,
    destination: &str,
    amount: u64,
) -> Result<TxResult, PaymentError> {
    self.execute_transfer(destination, amount).await
}
```

**Step 6: Update deposit_address method**

```rust
fn deposit_address(&self) -> String {
    self.deposit_account.to_string()
}
```

**Step 7: Verify it compiles**

Run: `cargo check -p x402-payments`
Expected: Compiles successfully

**Step 8: Commit**

```bash
git add crates/x402-payments/src/chains/near.rs
git commit -m "feat(near): implement transfer_to with ed25519 signing"
```

---

## Task 4: Solana Chain - Implement Transaction Signing

**Files:**
- Modify: `crates/x402-payments/src/chains/solana.rs`

**Step 1: Add Solana SDK imports and update struct**

Add imports at the top:

```rust
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer as SolanaSigner},
    transaction::Transaction,
};
use solana_client::rpc_client::RpcClient;
use spl_associated_token_account::get_associated_token_address;
use spl_token::instruction::transfer_checked;
```

Update the struct:

```rust
pub struct SolanaFacilitator {
    config: SolanaChainConfig,
    /// TEE-derived keypair.
    keypair: Keypair,
    /// Deposit wallet public key.
    deposit_address: Pubkey,
    /// Solana RPC client.
    rpc_client: RpcClient,
    /// HTTP client for legacy calls.
    client: reqwest::Client,
}
```

**Step 2: Update derive_deposit_address to return keypair**

```rust
/// Derive deposit wallet keypair from TEE key.
///
/// Returns (keypair, pubkey) tuple.
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

    // Create keypair from 32-byte seed
    let seed: [u8; 32] = key_bytes[..32]
        .try_into()
        .map_err(|_| PaymentError::Internal("Key conversion failed".to_string()))?;

    let keypair = Keypair::from_seed(&seed)
        .map_err(|e| PaymentError::Internal(format!("Invalid keypair seed: {}", e)))?;

    let pubkey = keypair.pubkey();
    info!("Derived Solana deposit address: {}", pubkey);

    Ok((keypair, pubkey))
}
```

**Step 3: Update constructor**

```rust
/// Create a new Solana facilitator with TEE-derived wallet.
pub async fn new(
    config: SolanaChainConfig,
    dstack: &dstack_client::DstackClient,
) -> Result<Self, PaymentError> {
    let (keypair, deposit_address) = Self::derive_wallet(dstack).await?;

    info!(
        "Initializing Solana facilitator: rpc={}, usdc_mint={}, deposit={}",
        config.rpc_url, config.usdc_mint, deposit_address
    );

    let rpc_client = RpcClient::new_with_commitment(
        config.rpc_url.clone(),
        CommitmentConfig::confirmed(),
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| PaymentError::Internal(format!("Failed to create HTTP client: {}", e)))?;

    Ok(Self {
        config,
        keypair,
        deposit_address,
        rpc_client,
        client,
    })
}
```

**Step 4: Fix get_deposit_wallet_balance**

```rust
async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError> {
    let usdc_mint: Pubkey = self.config.usdc_mint
        .parse()
        .map_err(|e| PaymentError::Internal(format!("Invalid USDC mint: {}", e)))?;

    // Get the associated token account for our deposit address
    let ata = get_associated_token_address(&self.deposit_address, &usdc_mint);

    // Query token account balance
    match self.rpc_client.get_token_account_balance(&ata) {
        Ok(balance) => {
            let amount = balance.amount.parse::<u64>().unwrap_or(0);
            debug!("Solana deposit wallet balance: {} USDC (micro)", amount);
            Ok(amount)
        }
        Err(e) => {
            // Account may not exist yet (no deposits)
            debug!("Solana ATA not found or error: {}", e);
            Ok(0)
        }
    }
}
```

**Step 5: Implement transfer_to**

```rust
/// Transfer USDC (SPL token) from deposit wallet to destination.
async fn execute_transfer(
    &self,
    destination: &str,
    amount: u64,
) -> Result<TxResult, PaymentError> {
    let dest_pubkey: Pubkey = destination
        .parse()
        .map_err(|e| PaymentError::Internal(format!("Invalid destination address: {}", e)))?;

    let usdc_mint: Pubkey = self.config.usdc_mint
        .parse()
        .map_err(|e| PaymentError::Internal(format!("Invalid USDC mint: {}", e)))?;

    // Get associated token accounts
    let source_ata = get_associated_token_address(&self.deposit_address, &usdc_mint);
    let dest_ata = get_associated_token_address(&dest_pubkey, &usdc_mint);

    info!(
        "Sending Solana SPL transfer: {} -> {}, amount: {}",
        source_ata, dest_ata, amount
    );

    // Build transfer instruction (USDC has 6 decimals)
    let transfer_ix = transfer_checked(
        &spl_token::id(),
        &source_ata,
        &usdc_mint,
        &dest_ata,
        &self.deposit_address,
        &[],
        amount,
        6, // USDC decimals
    )
    .map_err(|e| PaymentError::Internal(format!("Failed to create transfer instruction: {}", e)))?;

    // Get recent blockhash
    let recent_blockhash = self.rpc_client
        .get_latest_blockhash()
        .map_err(|e| PaymentError::RpcError(format!("Failed to get blockhash: {}", e)))?;

    // Build and sign transaction
    let tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&self.deposit_address),
        &[&self.keypair],
        recent_blockhash,
    );

    // Send and confirm transaction
    let signature = self.rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| PaymentError::RpcError(format!("Failed to send transaction: {}", e)))?;

    let tx_hash = signature.to_string();
    info!("Solana transfer complete: {}", tx_hash);

    Ok(TxResult {
        tx_hash,
        block_number: None,
        success: true,
    })
}
```

**Step 6: Update ChainFacilitator impl**

```rust
async fn transfer_to(
    &self,
    destination: &str,
    amount: u64,
) -> Result<TxResult, PaymentError> {
    self.execute_transfer(destination, amount).await
}

fn deposit_address(&self) -> String {
    self.deposit_address.to_string()
}
```

**Step 7: Verify it compiles**

Run: `cargo check -p x402-payments`
Expected: Compiles successfully

**Step 8: Commit**

```bash
git add crates/x402-payments/src/chains/solana.rs
git commit -m "feat(solana): implement transfer_to with SPL token transfer"
```

---

## Task 5: Update AppState for Multi-Chain Support

**Files:**
- Modify: `crates/x402-payments/src/api/handlers.rs`

**Step 1: Add imports for NEAR and Solana facilitators**

```rust
use crate::chains::{
    base::BaseFacilitator,
    near::NearFacilitator,
    solana::SolanaFacilitator,
    ChainFacilitator, PaymentPayload,
};
```

**Step 2: Update AppState struct**

```rust
pub struct AppState {
    pub credit_store: Arc<CreditStore>,
    pub config: PaymentConfig,
    pub pricing: PricingCalculator,
    pub base: Option<Arc<BaseFacilitator>>,
    pub near: Option<Arc<NearFacilitator>>,
    pub solana: Option<Arc<SolanaFacilitator>>,
}
```

**Step 3: Update AppState::new**

```rust
impl AppState {
    pub fn new(
        credit_store: Arc<CreditStore>,
        config: PaymentConfig,
        base: Option<Arc<BaseFacilitator>>,
        near: Option<Arc<NearFacilitator>>,
        solana: Option<Arc<SolanaFacilitator>>,
    ) -> Self {
        let pricing = PricingCalculator::new(config.pricing.clone());
        Self {
            credit_store,
            config,
            pricing,
            base,
            near,
            solana,
        }
    }
}
```

**Step 4: Update health_check handler**

Update the health check to use actual facilitator health:

```rust
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let base_healthy = if let Some(base) = &state.base {
        base.health_check().await.unwrap_or(false)
    } else {
        false
    };

    let near_healthy = if let Some(near) = &state.near {
        near.health_check().await.unwrap_or(false)
    } else {
        false
    };

    let solana_healthy = if let Some(solana) = &state.solana {
        solana.health_check().await.unwrap_or(false)
    } else {
        false
    };

    let chains = vec![
        ChainHealth {
            chain: Chain::Base,
            enabled: state.base.is_some(),
            healthy: base_healthy,
        },
        ChainHealth {
            chain: Chain::Near,
            enabled: state.near.is_some(),
            healthy: near_healthy,
        },
        ChainHealth {
            chain: Chain::Solana,
            enabled: state.solana.is_some(),
            healthy: solana_healthy,
        },
    ];

    Json(HealthResponse {
        healthy: base_healthy || near_healthy || solana_healthy || chains.iter().all(|c| !c.enabled),
        payments_enabled: state.config.enabled,
        chains,
    })
}
```

**Step 5: Update process_deposit handler**

Replace the match block for chain verification:

```rust
let verification = match request.chain {
    Chain::Base => {
        let facilitator = state.base.as_ref().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Base facilitator not initialized", "INTERNAL_ERROR")),
            )
        })?;
        facilitator.verify_payment(&payload).await
    }
    Chain::Near => {
        let facilitator = state.near.as_ref().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("NEAR facilitator not initialized", "INTERNAL_ERROR")),
            )
        })?;
        facilitator.verify_payment(&payload).await
    }
    Chain::Solana => {
        let facilitator = state.solana.as_ref().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Solana facilitator not initialized", "INTERNAL_ERROR")),
            )
        })?;
        facilitator.verify_payment(&payload).await
    }
}
.map_err(|e| {
    warn!("Payment verification failed: {}", e);
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new(e.to_string(), "VERIFICATION_FAILED")),
    )
})?;
```

**Step 6: Update get_deposit_address handler**

Update to use real facilitator addresses:

```rust
let (address, token_contract) = match chain {
    Chain::Base => {
        let facilitator = state.base.as_ref().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("Base not configured", "CHAIN_DISABLED")),
            )
        })?;
        let config = state.config.base.as_ref().unwrap();
        (facilitator.deposit_address(), config.usdc_contract.clone())
    }
    Chain::Near => {
        let facilitator = state.near.as_ref().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("NEAR not configured", "CHAIN_DISABLED")),
            )
        })?;
        let config = state.config.near.as_ref().unwrap();
        (facilitator.deposit_address(), config.usdc_contract.clone())
    }
    Chain::Solana => {
        let facilitator = state.solana.as_ref().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("Solana not configured", "CHAIN_DISABLED")),
            )
        })?;
        let config = state.config.solana.as_ref().unwrap();
        (facilitator.deposit_address(), config.usdc_mint.clone())
    }
};
```

**Step 7: Verify it compiles**

Run: `cargo check -p x402-payments`
Expected: Compiles successfully

**Step 8: Commit**

```bash
git add crates/x402-payments/src/api/handlers.rs
git commit -m "feat(api): add NEAR and Solana facilitators to AppState"
```

---

## Task 6: Update Server Initialization

**Files:**
- Modify: `crates/x402-payments/src/lib.rs`

**Step 1: Add imports**

```rust
use chains::near::NearFacilitator;
use chains::solana::SolanaFacilitator;
```

**Step 2: Update spawn_payment_server function**

Replace the facilitator initialization section:

```rust
pub async fn spawn_payment_server(
    config: PaymentConfig,
    dstack: DstackClient,
) -> Result<Option<tokio::task::JoinHandle<Result<(), PaymentError>>>, PaymentError> {
    if !config.enabled {
        info!("Payments disabled");
        return Ok(None);
    }

    let credit_store = CreditStore::new(dstack.clone(), config.storage_path.clone()).await?;

    // Initialize Base facilitator
    let base_facilitator = if let Some(base_config) = &config.base {
        if base_config.enabled {
            match BaseFacilitator::new(base_config.clone(), &dstack).await {
                Ok(facilitator) => {
                    info!("Base facilitator initialized");
                    Some(Arc::new(facilitator))
                }
                Err(e) => {
                    warn!("Failed to initialize Base facilitator: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Initialize NEAR facilitator
    let near_facilitator = if let Some(near_config) = &config.near {
        if near_config.enabled {
            match NearFacilitator::new(near_config.clone(), &dstack).await {
                Ok(facilitator) => {
                    info!("NEAR facilitator initialized");
                    Some(Arc::new(facilitator))
                }
                Err(e) => {
                    warn!("Failed to initialize NEAR facilitator: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Initialize Solana facilitator
    let solana_facilitator = if let Some(sol_config) = &config.solana {
        if sol_config.enabled {
            match SolanaFacilitator::new(sol_config.clone(), &dstack).await {
                Ok(facilitator) => {
                    info!("Solana facilitator initialized");
                    Some(Arc::new(facilitator))
                }
                Err(e) => {
                    warn!("Failed to initialize Solana facilitator: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Collect facilitators for sweeper
    let mut chain_facilitators: Vec<Arc<dyn ChainFacilitator>> = Vec::new();
    if let Some(ref base) = base_facilitator {
        chain_facilitators.push(base.clone());
    }
    if let Some(ref near) = near_facilitator {
        chain_facilitators.push(near.clone());
    }
    if let Some(ref solana) = solana_facilitator {
        chain_facilitators.push(solana.clone());
    }

    // Spawn sweeper if any chains have operator addresses configured
    let operator_addresses = config.operator_addresses();
    if !chain_facilitators.is_empty() && operator_addresses.has_any() {
        info!(
            "Spawning fund sweeper with {} chains",
            chain_facilitators.len()
        );
        spawn_sweeper(chain_facilitators, operator_addresses, config.sweep.clone());
    }

    let state = Arc::new(AppState::new(
        credit_store,
        config.clone(),
        base_facilitator,
        near_facilitator,
        solana_facilitator,
    ));
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
```

**Step 3: Also update start_payment_server similarly**

Apply the same pattern to the `start_payment_server` function.

**Step 4: Add ChainFacilitator import**

```rust
use chains::ChainFacilitator;
```

**Step 5: Verify it compiles**

Run: `cargo check -p x402-payments`
Expected: Compiles successfully

**Step 6: Run tests**

Run: `cargo test -p x402-payments`
Expected: All tests pass

**Step 7: Commit**

```bash
git add crates/x402-payments/src/lib.rs
git commit -m "feat: initialize all chain facilitators and wire up sweeper"
```

---

## Task 7: Update Tests

**Files:**
- Modify: `crates/x402-payments/src/chains/base.rs` (tests section)
- Modify: `crates/x402-payments/src/chains/near.rs` (tests section)
- Modify: `crates/x402-payments/src/chains/solana.rs` (tests section)

**Step 1: Update Base tests**

Since `BaseFacilitator::new` now requires a dstack client, update the test:

```rust
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
        assert_eq!(parse_hex_u64("0xf4240").unwrap(), 1_000_000);
    }

    #[test]
    fn test_parse_hex_large() {
        assert_eq!(parse_hex_u64("0x3b9aca00").unwrap(), 1_000_000_000);
    }

    #[test]
    fn test_chain_type() {
        // Simple test that doesn't require dstack
        assert_eq!(Chain::Base.to_string(), "Base");
    }
}
```

**Step 2: Update NEAR tests similarly**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_type() {
        assert_eq!(Chain::Near.to_string(), "NEAR");
    }
}
```

**Step 3: Update Solana tests similarly**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_type() {
        assert_eq!(Chain::Solana.to_string(), "Solana");
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p x402-payments`
Expected: All tests pass

**Step 5: Commit**

```bash
git add crates/x402-payments/src/chains/
git commit -m "test: update chain facilitator tests for new constructors"
```

---

## Task 8: Final Integration Test

**Step 1: Build the entire project**

Run: `cargo build`
Expected: Compiles successfully

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Verify no clippy warnings**

Run: `cargo clippy -p x402-payments -- -D warnings`
Expected: No warnings

**Step 4: Final commit with summary**

```bash
git add -A
git commit -m "feat: complete fund sweeping and multi-chain support

- Implement transfer_to for Base using alloy wallet signing
- Implement transfer_to for NEAR using near-crypto ed25519 signing
- Implement transfer_to for Solana using solana-sdk SPL token transfer
- Wire up all three facilitators in server startup
- Connect facilitators to FundSweeper for automatic deposit sweeping
- Update API handlers to use appropriate facilitator per chain

All chains now support:
- TEE-derived deterministic keypairs
- On-chain payment verification
- Automatic fund sweeping to operator wallets"
```

---

## Summary

| Task | Description | Files Changed |
|------|-------------|---------------|
| 1 | Add dependencies | Cargo.toml |
| 2 | Base chain transfer_to | chains/base.rs |
| 3 | NEAR chain transfer_to | chains/near.rs |
| 4 | Solana chain transfer_to | chains/solana.rs |
| 5 | Update AppState | api/handlers.rs |
| 6 | Server initialization | lib.rs |
| 7 | Update tests | chains/*.rs |
| 8 | Final integration | All |

**Total: 8 tasks, ~800-900 lines of code**

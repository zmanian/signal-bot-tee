# Fund Sweeping & Multi-Chain Support Design

**Date**: 2024-12-21
**Status**: Approved

## Overview

Implement fund sweeping (`transfer_to` method) for all three chain facilitators and wire up NEAR/Solana facilitators in server startup.

## Goals

1. Enable automatic transfer of accumulated deposits from TEE deposit wallets to operator wallets
2. Support deposit verification and sweeping for Base, NEAR, and Solana chains
3. Maintain security: private keys only exist in TEE memory, derived deterministically

## Dependencies

```toml
# EVM (Base)
alloy = { version = "0.9", features = ["providers", "signers", "network"] }

# NEAR
near-crypto = "0.27"
near-primitives = "0.27"
near-jsonrpc-client = "0.13"
near-jsonrpc-primitives = "0.27"

# Solana
solana-sdk = "2.1"
solana-client = "2.1"
spl-token = "7"
spl-associated-token-account = "5"
```

## Key Derivation

Each facilitator derives a proper keypair from TEE entropy:

| Chain | Curve | Derivation Path |
|-------|-------|-----------------|
| Base | secp256k1 | `x402-payments/base-deposit-wallet` |
| NEAR | ed25519 | `x402-payments/near-deposit-wallet` |
| Solana | ed25519 | `x402-payments/solana-deposit-wallet` |

Pattern:
1. Call `dstack.derive_key(path, None)` to get 32 bytes of TEE-derived entropy
2. Use entropy as private key seed for the appropriate curve
3. Derive public key/address from keypair
4. Store keypair in facilitator struct (wrapped in `secrecy::SecretBox`)

## Transaction Signing

### Base (EVM) - Using Alloy

```rust
struct BaseFacilitator {
    config: BaseChainConfig,
    wallet: LocalWallet,  // alloy signer with secp256k1 key
    provider: RootProvider<Http<Client>>,
    deposit_address: Address,
}

async fn transfer_to(&self, destination: &str, amount: u64) -> Result<TxResult> {
    // 1. Build ERC20 transfer calldata
    let call = transferCall { to: destination.parse()?, value: U256::from(amount) };

    // 2. Create transaction
    let tx = TransactionRequest::default()
        .to(self.config.usdc_contract.parse()?)
        .input(call.abi_encode().into());

    // 3. Send (alloy handles signing, nonce, gas estimation)
    let pending = self.provider.send_transaction(tx).await?;
    let receipt = pending.get_receipt().await?;

    Ok(TxResult { tx_hash: receipt.transaction_hash.to_string(), ... })
}
```

### NEAR - Using near-jsonrpc-client

```rust
struct NearFacilitator {
    config: NearChainConfig,
    signer: InMemorySigner,  // ed25519 keypair
    client: JsonRpcClient,
    deposit_account: AccountId,
}

async fn transfer_to(&self, destination: &str, amount: u64) -> Result<TxResult> {
    // 1. Build ft_transfer action
    let args = json!({ "receiver_id": destination, "amount": amount.to_string() });
    let action = Action::FunctionCall(FunctionCallAction {
        method_name: "ft_transfer".into(),
        args: args.to_string().into_bytes(),
        gas: 30_000_000_000_000,  // 30 TGas
        deposit: 1,  // 1 yoctoNEAR for ft_transfer
    });

    // 2. Get access key nonce, build and sign transaction
    // 3. Send via broadcast_tx_commit
}
```

### Solana - Using solana-sdk

```rust
struct SolanaFacilitator {
    config: SolanaChainConfig,
    keypair: Keypair,  // ed25519
    client: RpcClient,
    deposit_address: Pubkey,
}

async fn transfer_to(&self, destination: &str, amount: u64) -> Result<TxResult> {
    // 1. Derive ATAs for source and destination
    let source_ata = get_associated_token_address(&self.deposit_address, &usdc_mint);
    let dest_ata = get_associated_token_address(&destination.parse()?, &usdc_mint);

    // 2. Build transfer instruction
    let ix = spl_token::instruction::transfer_checked(...)?;

    // 3. Create, sign, and send transaction
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&self.deposit_address), ...);
    let sig = self.client.send_and_confirm_transaction(&tx)?;
}
```

## Multi-Chain Server Startup

### AppState Changes

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

### Server Initialization

```rust
pub async fn spawn_payment_server(config: PaymentConfig, dstack: DstackClient) -> Result<...> {
    let base_facilitator = if config.base.as_ref().is_some_and(|c| c.enabled) {
        Some(Arc::new(BaseFacilitator::new(&config.base.unwrap(), &dstack).await?))
    } else { None };

    let near_facilitator = if config.near.as_ref().is_some_and(|c| c.enabled) {
        Some(Arc::new(NearFacilitator::new(&config.near.unwrap(), &dstack).await?))
    } else { None };

    let solana_facilitator = if config.solana.as_ref().is_some_and(|c| c.enabled) {
        Some(Arc::new(SolanaFacilitator::new(&config.solana.unwrap(), &dstack).await?))
    } else { None };

    let state = Arc::new(AppState::new(
        credit_store, config, base_facilitator, near_facilitator, solana_facilitator
    ));
    // ...
}
```

## Sweeper Integration

```rust
// Collect enabled facilitators for sweeper
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
    spawn_sweeper(chain_facilitators, operator_addresses, config.sweep.clone());
}
```

## Implementation Order

1. Add dependencies to Cargo.toml
2. Base chain - Fix key derivation, implement `transfer_to`
3. NEAR chain - Fix key derivation, implement `transfer_to`
4. Solana chain - Fix key derivation, ATA handling, implement `transfer_to`
5. Multi-chain startup - Update AppState, lib.rs, handlers.rs
6. Sweeper wiring - Connect facilitators to sweeper
7. Tests - Unit tests for each chain's transfer logic

## Estimated Scope

- ~200 lines per chain for transfer logic
- ~100 lines for multi-chain wiring
- ~150 lines for tests
- **Total: ~800-900 lines of code**

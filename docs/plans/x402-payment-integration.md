# x402 Payment Integration for Signal Bot TEE

## Overview

Integrate x402 prepaid credit payments so users can deposit USDC (on Base, NEAR, Solana) and have credits deducted per-message based on token usage.

## Architecture

```
User deposits USDC → HTTP API (port 8082) → Verify on-chain → Credit balance
User sends message → Check balance → Process → Deduct credits → Respond
```

## New Crate: `crates/x402-payments/`

```
src/
├── lib.rs                 # Module exports
├── error.rs               # PaymentError enum
├── config.rs              # PaymentConfig struct
├── types.rs               # CreditBalance, Deposit, UsageRecord
├── credits/
│   ├── balance.rs         # Credit operations
│   ├── pricing.rs         # Token-to-credit calculation
│   └── store.rs           # TEE-encrypted persistence
├── chains/
│   ├── mod.rs             # ChainFacilitator trait
│   ├── base.rs            # EVM via x402-rs
│   ├── near.rs            # NEAR RPC verification
│   └── solana.rs          # via x402-sdk-solana-rust
└── api/
    ├── handlers.rs        # Axum HTTP handlers
    └── types.rs           # Request/response DTOs
```

## Key Components

### 1. Credit Store (TEE-Encrypted)

Follow pattern from `signal-registration-proxy/src/registry/encrypted.rs`:
- Key derivation: `dstack.derive_key("x402-payments/credit-store")`
- AES-256-GCM encryption
- Storage: `/data/credits.enc`

```rust
pub struct CreditBalance {
    pub user_id: String,              // Phone number
    pub credits_remaining: u64,        // Micro-USDC (1e-6)
    pub total_deposited: u64,
    pub total_consumed: u64,
}
```

### 2. Pricing

```rust
pub struct PricingConfig {
    pub prompt_credits_per_million: u64,      // Default: 100,000 (=$0.10)
    pub completion_credits_per_million: u64,  // Default: 300,000 (=$0.30)
    pub minimum_credits_per_message: u64,     // Default: 100 (=$0.0001)
    pub usdc_to_credits_ratio: u64,           // 1 USDC = 1,000,000 credits
}
```

### 3. Chain Facilitators

| Chain | Library | Verification Method |
|-------|---------|---------------------|
| Base | `x402-rs` | x402 protocol (verify/settle) |
| NEAR | `near-jsonrpc-client` | Query tx, verify ft_transfer memo |
| Solana | `x402-sdk-solana-rust` | x402 protocol |

### 4. HTTP API (Port 8082)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/balance/{phone}` | Get credit balance |
| `POST` | `/v1/deposit` | Process payment payload |
| `GET` | `/v1/deposit-address/{chain}` | Get deposit address |
| `GET` | `/v1/pricing` | Get pricing config |

### 5. Signal Commands

**`!balance`** - Show credit balance
```
Your Balance:
Credits: 1,500,000 ($1.50 USDC)
Total Deposited: $5.00 USDC
Total Used: $3.50 USDC
```

**`!deposit`** - Show deposit addresses for each chain

## Integration Points

### ChatHandler Modification (`crates/signal-bot/src/commands/chat.rs`)

```rust
// Before processing:
if !credit_store.has_credits(&user_id, estimated_cost).await {
    return Ok("Insufficient credits. Use `!deposit` to add USDC.");
}

// After NEAR AI response:
let actual_cost = calculate_credits(&response.usage, &pricing_config);
credit_store.deduct_credits(&user_id, actual_cost, usage_record).await?;
```

### NEAR AI Client Change (`crates/near-ai-client/src/types.rs`)

Ensure `ChatResponse` includes `usage: Option<Usage>` and propagate it through `chat_with_tools()`.

## Files to Modify

| File | Changes |
|------|---------|
| `crates/signal-bot/src/main.rs` | Add payment server, credit store init |
| `crates/signal-bot/src/commands/mod.rs` | Add balance, deposit handlers |
| `crates/signal-bot/src/commands/chat.rs` | Add credit check/deduction |
| `crates/near-ai-client/src/client.rs` | Return usage from responses |
| `docker/phala-compose.yaml` | Add port 8082, credits-data volume |
| `Cargo.toml` (workspace) | Add x402-payments crate |

## Dependencies

```toml
[dependencies]
# Core x402
x402-rs = "0.10"
x402-axum = "0.1"

# Solana
x402-sdk-solana-rust = "0.1"

# NEAR
near-jsonrpc-client = "0.8"

# Encryption (same as registration-proxy)
aes-gcm = "0.10"
rand = "0.8"
```

## Configuration (Environment Variables)

```yaml
PAYMENTS__ENABLED=true
PAYMENTS__SERVER_PORT=8082
PAYMENTS__PROMPT_CREDITS_PER_MILLION=100000
PAYMENTS__COMPLETION_CREDITS_PER_MILLION=300000

# Base
PAYMENTS__BASE__RPC_URL=https://mainnet.base.org
PAYMENTS__BASE__USDC_CONTRACT=0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
PAYMENTS__BASE__DEPOSIT_ADDRESS=0x...

# NEAR
PAYMENTS__NEAR__RPC_URL=https://rpc.mainnet.near.org
PAYMENTS__NEAR__DEPOSIT_ACCOUNT=signal-bot.near

# Solana
PAYMENTS__SOLANA__RPC_URL=https://api.mainnet-beta.solana.com
PAYMENTS__SOLANA__USDC_MINT=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
```

## Fund Flow: Operator Withdrawal

### Architecture

```
User deposits USDC → TEE Deposit Wallet → Automatic Sweep → Operator Wallet
                          ↓
                    Credit balance updated
```

### Design

1. **TEE-Controlled Deposit Wallets**
   - Keys derived via `dstack.derive_key("x402-payments/{chain}-deposit-wallet")`
   - Never exposed outside TEE
   - Users deposit to these addresses

2. **Automatic Sweep to Operator**
   - Background task runs every N hours (configurable, default: 24h)
   - Sweeps accumulated deposits to operator's configured withdrawal address
   - Leaves minimum reserve for gas fees

3. **Attestable Operator Address**
   - Operator withdrawal addresses configured in `docker-compose.yaml`
   - Users can verify via `!verify` → compose hash proves operator addresses
   - Transparent: "funds go to 0x... on Base, signal-bot-operator.near on NEAR"

### Configuration

```yaml
# In phala-compose.yaml (attestable)
environment:
  # Operator withdrawal addresses
  - PAYMENTS__BASE__OPERATOR_ADDRESS=0xYourBaseAddress
  - PAYMENTS__NEAR__OPERATOR_ACCOUNT=your-account.near
  - PAYMENTS__SOLANA__OPERATOR_ADDRESS=YourSolanaPubkey

  # Sweep settings
  - PAYMENTS__SWEEP_INTERVAL_HOURS=24
  - PAYMENTS__SWEEP_MIN_AMOUNT_USDC=10  # Min amount to trigger sweep
  - PAYMENTS__SWEEP_RESERVE_FOR_GAS=0.01  # Keep for gas
```

### Sweep Implementation

```rust
pub struct FundSweeper {
    chains: Vec<Arc<dyn ChainFacilitator>>,
    operator_addresses: OperatorAddresses,
    interval: Duration,
    min_sweep_amount: u64,
}

impl FundSweeper {
    /// Background task that runs periodically
    pub async fn run(&self) {
        loop {
            tokio::time::sleep(self.interval).await;

            for chain in &self.chains {
                if let Err(e) = self.sweep_chain(chain).await {
                    error!("Sweep failed for {:?}: {}", chain.chain(), e);
                }
            }
        }
    }

    async fn sweep_chain(&self, chain: &dyn ChainFacilitator) -> Result<(), PaymentError> {
        let balance = chain.get_deposit_wallet_balance().await?;

        if balance >= self.min_sweep_amount {
            let operator_addr = self.operator_addresses.get(chain.chain());
            let tx = chain.transfer_to(operator_addr, balance - self.reserve_for_gas).await?;
            info!("Swept {} USDC to {} on {:?}, tx: {}",
                  balance, operator_addr, chain.chain(), tx.hash);
        }

        Ok(())
    }
}
```

### User-Facing Transparency

**`!verify` command updated output:**
```
TEE Attestation:
...
Operator Fund Addresses:
  Base: 0x1234...
  NEAR: operator.near
  Solana: ABC123...

Deposits are automatically swept to these addresses every 24h.
Verify these match the expected operator.
```

### ChainFacilitator Trait Extension

```rust
#[async_trait]
pub trait ChainFacilitator: Send + Sync {
    // ... existing methods ...

    /// Get current balance of TEE deposit wallet
    async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError>;

    /// Transfer USDC from deposit wallet to destination
    async fn transfer_to(&self, destination: &str, amount: u64) -> Result<TxResult, PaymentError>;
}
```

## Security Considerations

1. **Wallet keys derived in TEE** via `dstack.derive_key()` - never in config
2. **Double-spend prevention**: Track processed tx hashes
3. **Credit store encrypted** with TEE-derived AES key
4. **Rate limiting** on deposit API (10/hour per user)
5. **Operator addresses attestable** via compose hash verification
6. **Sweep transactions logged** for audit trail

## Implementation Phases

### Phase 1: Core Credit System
- [ ] Create `x402-payments` crate with types/errors
- [ ] Implement `CreditStore` with TEE encryption
- [ ] Add `!balance` command
- [ ] Add pricing calculation

### Phase 2: Base (EVM) Chain
- [ ] Integrate `x402-rs` for verification/settlement
- [ ] Implement `BaseFacilitator`
- [ ] Add HTTP deposit endpoint
- [ ] Add Base to `!deposit` command

### Phase 3: ChatHandler Integration
- [ ] Modify `near-ai-client` to return usage stats
- [ ] Add pre-flight balance check
- [ ] Add post-response credit deduction
- [ ] Append cost to responses

### Phase 4: NEAR Protocol
- [ ] Implement `NearFacilitator` with RPC client
- [ ] Verify USDC transfers via memo matching
- [ ] Add NEAR to `!deposit` command

### Phase 5: Solana
- [ ] Integrate `x402-sdk-solana-rust`
- [ ] Implement `SolanaFacilitator`
- [ ] Add Solana to `!deposit` command

### Phase 6: Fund Sweep
- [ ] Implement `FundSweeper` background task
- [ ] Add `get_deposit_wallet_balance()` and `transfer_to()` to each chain
- [ ] Update `!verify` to show operator addresses
- [ ] Add sweep transaction logging

### Phase 7: Testing
- [ ] Unit tests for credit operations
- [ ] Integration tests for each chain
- [ ] Sweep integration tests (testnet)
- [ ] Docker deployment testing

## Sources

- [x402-rs GitHub](https://github.com/x402-rs/x402-rs)
- [x402 Protocol Spec](https://www.x402.org/)
- [Coinbase x402 Docs](https://docs.cdp.coinbase.com/x402/welcome)
- [x402-sdk-solana-rust](https://crates.io/crates/x402-sdk-solana-rust)

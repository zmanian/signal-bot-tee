//! Payment configuration.

use crate::types::{Chain, OperatorAddresses};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

/// Main payment configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PaymentConfig {
    /// Whether payments are enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// HTTP server port for payment API.
    #[serde(default = "default_server_port")]
    pub server_port: u16,

    /// Pricing configuration.
    #[serde(default)]
    pub pricing: PricingConfig,

    /// Storage path for encrypted credit store.
    #[serde(default = "default_storage_path")]
    pub storage_path: PathBuf,

    /// Base (EVM) chain configuration.
    pub base: Option<BaseChainConfig>,

    /// NEAR Protocol configuration.
    pub near: Option<NearChainConfig>,

    /// Solana configuration.
    pub solana: Option<SolanaChainConfig>,

    /// Sweep configuration.
    #[serde(default)]
    pub sweep: SweepConfig,
}

fn default_enabled() -> bool {
    false
}

fn default_server_port() -> u16 {
    8082
}

fn default_storage_path() -> PathBuf {
    PathBuf::from("/data/credits.enc")
}

impl Default for PaymentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_port: 8082,
            pricing: PricingConfig::default(),
            storage_path: default_storage_path(),
            base: None,
            near: None,
            solana: None,
            sweep: SweepConfig::default(),
        }
    }
}

impl PaymentConfig {
    /// Get operator addresses from chain configs.
    pub fn operator_addresses(&self) -> OperatorAddresses {
        OperatorAddresses {
            base: self.base.as_ref().and_then(|c| c.operator_address.clone()),
            near: self.near.as_ref().and_then(|c| c.operator_account.clone()),
            solana: self.solana.as_ref().and_then(|c| c.operator_address.clone()),
        }
    }

    /// Get enabled chains.
    pub fn enabled_chains(&self) -> Vec<Chain> {
        let mut chains = Vec::new();
        if self.base.as_ref().is_some_and(|c| c.enabled) {
            chains.push(Chain::Base);
        }
        if self.near.as_ref().is_some_and(|c| c.enabled) {
            chains.push(Chain::Near);
        }
        if self.solana.as_ref().is_some_and(|c| c.enabled) {
            chains.push(Chain::Solana);
        }
        chains
    }
}

/// Pricing configuration for token-to-credit conversion.
#[derive(Debug, Clone, Deserialize)]
pub struct PricingConfig {
    /// Credits per 1M prompt tokens.
    /// Default: 100,000 (= $0.10 per 1M tokens)
    #[serde(default = "default_prompt_credits")]
    pub prompt_credits_per_million: u64,

    /// Credits per 1M completion tokens.
    /// Default: 300,000 (= $0.30 per 1M tokens)
    #[serde(default = "default_completion_credits")]
    pub completion_credits_per_million: u64,

    /// Minimum credits per message (floor).
    /// Default: 100 (= $0.0001)
    #[serde(default = "default_minimum_credits")]
    pub minimum_credits_per_message: u64,

    /// USDC to credits ratio.
    /// Default: 1,000,000 (1 USDC = 1M credits)
    #[serde(default = "default_usdc_ratio")]
    pub usdc_to_credits_ratio: u64,
}

fn default_prompt_credits() -> u64 {
    100_000
}

fn default_completion_credits() -> u64 {
    300_000
}

fn default_minimum_credits() -> u64 {
    100
}

fn default_usdc_ratio() -> u64 {
    1_000_000
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            prompt_credits_per_million: default_prompt_credits(),
            completion_credits_per_million: default_completion_credits(),
            minimum_credits_per_message: default_minimum_credits(),
            usdc_to_credits_ratio: default_usdc_ratio(),
        }
    }
}

/// Base (EVM) chain configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct BaseChainConfig {
    /// Whether Base chain is enabled.
    #[serde(default = "default_chain_enabled")]
    pub enabled: bool,

    /// RPC URL for Base.
    #[serde(default = "default_base_rpc")]
    pub rpc_url: String,

    /// USDC contract address on Base.
    #[serde(default = "default_base_usdc")]
    pub usdc_contract: String,

    /// Operator's withdrawal address.
    pub operator_address: Option<String>,
}

fn default_chain_enabled() -> bool {
    true
}

fn default_base_rpc() -> String {
    "https://mainnet.base.org".to_string()
}

fn default_base_usdc() -> String {
    // Base USDC contract
    "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()
}

/// NEAR Protocol configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct NearChainConfig {
    /// Whether NEAR is enabled.
    #[serde(default = "default_chain_enabled")]
    pub enabled: bool,

    /// RPC URL for NEAR.
    #[serde(default = "default_near_rpc")]
    pub rpc_url: String,

    /// USDC contract on NEAR (bridged from Circle).
    #[serde(default = "default_near_usdc")]
    pub usdc_contract: String,

    /// Operator's withdrawal account.
    pub operator_account: Option<String>,
}

fn default_near_rpc() -> String {
    "https://rpc.mainnet.near.org".to_string()
}

fn default_near_usdc() -> String {
    // USDC on NEAR (bridged)
    "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1".to_string()
}

/// Solana configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SolanaChainConfig {
    /// Whether Solana is enabled.
    #[serde(default = "default_chain_enabled")]
    pub enabled: bool,

    /// RPC URL for Solana.
    #[serde(default = "default_solana_rpc")]
    pub rpc_url: String,

    /// USDC mint address on Solana.
    #[serde(default = "default_solana_usdc")]
    pub usdc_mint: String,

    /// Operator's withdrawal address.
    pub operator_address: Option<String>,
}

fn default_solana_rpc() -> String {
    "https://api.mainnet-beta.solana.com".to_string()
}

fn default_solana_usdc() -> String {
    // USDC on Solana
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()
}

/// Fund sweep configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SweepConfig {
    /// Interval between sweep runs.
    #[serde(default = "default_sweep_interval", with = "humantime_serde")]
    pub interval: Duration,

    /// Minimum amount to trigger a sweep (in micro-USDC).
    #[serde(default = "default_min_sweep_amount")]
    pub min_amount_usdc: u64,

    /// Reserve to keep for gas fees (in micro-USDC).
    #[serde(default = "default_reserve_for_gas")]
    pub reserve_for_gas: u64,
}

fn default_sweep_interval() -> Duration {
    Duration::from_secs(24 * 60 * 60) // 24 hours
}

fn default_min_sweep_amount() -> u64 {
    10_000_000 // 10 USDC
}

fn default_reserve_for_gas() -> u64 {
    10_000 // $0.01 for gas
}

impl Default for SweepConfig {
    fn default() -> Self {
        Self {
            interval: default_sweep_interval(),
            min_amount_usdc: default_min_sweep_amount(),
            reserve_for_gas: default_reserve_for_gas(),
        }
    }
}

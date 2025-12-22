//! Fund sweeper for automatic deposit-to-operator transfers.
//!
//! Periodically checks deposit wallet balances and transfers accumulated
//! funds to the operator's withdrawal address.

use crate::chains::ChainFacilitator;
use crate::config::SweepConfig;
use crate::error::PaymentError;
use crate::types::{Chain, OperatorAddresses, SweepRecord};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Fund sweeper that periodically transfers deposits to operator wallets.
pub struct FundSweeper {
    /// Chain facilitators for each enabled chain.
    chains: Vec<Arc<dyn ChainFacilitator>>,
    /// Operator addresses for each chain.
    operator_addresses: OperatorAddresses,
    /// Sweep configuration.
    config: SweepConfig,
    /// Sweep history (in-memory for now).
    sweep_history: tokio::sync::RwLock<Vec<SweepRecord>>,
}

impl FundSweeper {
    /// Create a new fund sweeper.
    pub fn new(
        chains: Vec<Arc<dyn ChainFacilitator>>,
        operator_addresses: OperatorAddresses,
        config: SweepConfig,
    ) -> Self {
        Self {
            chains,
            operator_addresses,
            config,
            sweep_history: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Get the operator address for a chain.
    fn get_operator_address(&self, chain: Chain) -> Option<&str> {
        match chain {
            Chain::Base => self.operator_addresses.base.as_deref(),
            Chain::Near => self.operator_addresses.near.as_deref(),
            Chain::Solana => self.operator_addresses.solana.as_deref(),
        }
    }

    /// Run a single sweep cycle across all chains.
    pub async fn sweep_once(&self) -> Vec<SweepRecord> {
        let mut records = Vec::new();

        for chain in &self.chains {
            match self.sweep_chain(chain.as_ref()).await {
                Ok(Some(record)) => {
                    records.push(record);
                }
                Ok(None) => {
                    debug!("No sweep needed for {:?}", chain.chain());
                }
                Err(e) => {
                    error!("Sweep failed for {:?}: {}", chain.chain(), e);
                }
            }
        }

        // Store in history
        if !records.is_empty() {
            let mut history = self.sweep_history.write().await;
            history.extend(records.clone());
            // Keep last 100 records
            if history.len() > 100 {
                let keep_from = history.len() - 100;
                *history = history.drain(keep_from..).collect();
            }
        }

        records
    }

    /// Sweep funds from a single chain's deposit wallet.
    async fn sweep_chain(
        &self,
        chain: &dyn ChainFacilitator,
    ) -> Result<Option<SweepRecord>, PaymentError> {
        let chain_id = chain.chain();
        let deposit_address = chain.deposit_address();

        // Check if we have an operator address for this chain
        let operator_addr = match self.get_operator_address(chain_id) {
            Some(addr) => addr,
            None => {
                debug!("No operator address configured for {:?}", chain_id);
                return Ok(None);
            }
        };

        // Get deposit wallet balance
        let balance = chain.get_deposit_wallet_balance().await?;

        debug!(
            "{:?} deposit wallet ({}) balance: {} micro-USDC",
            chain_id, deposit_address, balance
        );

        // Check if balance exceeds sweep threshold
        if balance < self.config.min_amount_usdc {
            debug!(
                "{:?} balance ({}) below threshold ({}), skipping sweep",
                chain_id, balance, self.config.min_amount_usdc
            );
            return Ok(None);
        }

        // Calculate amount to sweep (leave reserve for gas)
        let sweep_amount = balance.saturating_sub(self.config.reserve_for_gas);

        if sweep_amount == 0 {
            debug!("{:?} sweep amount is zero after gas reserve", chain_id);
            return Ok(None);
        }

        info!(
            "Sweeping {} micro-USDC from {:?} ({}) to operator ({})",
            sweep_amount, chain_id, deposit_address, operator_addr
        );

        // Execute transfer
        let tx_result = chain.transfer_to(operator_addr, sweep_amount).await?;

        let record = SweepRecord {
            chain: chain_id,
            from: deposit_address,
            to: operator_addr.to_string(),
            amount: sweep_amount,
            tx_hash: tx_result.tx_hash.clone(),
            success: tx_result.success,
            timestamp: chrono::Utc::now(),
        };

        if tx_result.success {
            info!(
                "Sweep successful: {:?} {} micro-USDC, tx: {}",
                chain_id, sweep_amount, tx_result.tx_hash
            );
        } else {
            warn!(
                "Sweep may have failed: {:?} tx: {}",
                chain_id, tx_result.tx_hash
            );
        }

        Ok(Some(record))
    }

    /// Run the sweeper as a background task.
    ///
    /// This will run indefinitely, sleeping between sweep cycles.
    pub async fn run(&self) {
        info!(
            "Starting fund sweeper, interval: {:?}, min_amount: {} micro-USDC",
            self.config.interval, self.config.min_amount_usdc
        );

        loop {
            // Wait for the configured interval
            tokio::time::sleep(self.config.interval).await;

            info!("Running sweep cycle...");
            let records = self.sweep_once().await;

            if records.is_empty() {
                debug!("No sweeps performed this cycle");
            } else {
                info!("Sweep cycle complete: {} transfers", records.len());
            }
        }
    }

    /// Get sweep history.
    pub async fn get_history(&self) -> Vec<SweepRecord> {
        self.sweep_history.read().await.clone()
    }

    /// Get configured sweep interval.
    pub fn interval(&self) -> Duration {
        self.config.interval
    }

    /// Get operator addresses.
    pub fn operator_addresses(&self) -> &OperatorAddresses {
        &self.operator_addresses
    }
}

/// Spawn the fund sweeper as a background task.
///
/// Returns a JoinHandle for the sweeper task.
pub fn spawn_sweeper(
    chains: Vec<Arc<dyn ChainFacilitator>>,
    operator_addresses: OperatorAddresses,
    config: SweepConfig,
) -> tokio::task::JoinHandle<()> {
    let sweeper = Arc::new(FundSweeper::new(chains, operator_addresses, config));

    tokio::spawn(async move {
        sweeper.run().await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::{PaymentPayload, PaymentVerification, TxResult};
    use crate::types::{SettlementResult, TxStatus};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Mock chain facilitator for testing.
    struct MockFacilitator {
        chain: Chain,
        deposit_address: String,
        balance: AtomicU64,
        transfer_success: bool,
    }

    impl MockFacilitator {
        fn new(chain: Chain, balance: u64, transfer_success: bool) -> Self {
            Self {
                chain,
                deposit_address: format!("deposit-{:?}", chain),
                balance: AtomicU64::new(balance),
                transfer_success,
            }
        }
    }

    #[async_trait]
    impl ChainFacilitator for MockFacilitator {
        fn chain(&self) -> Chain {
            self.chain
        }

        fn deposit_address(&self) -> String {
            self.deposit_address.clone()
        }

        async fn verify_payment(
            &self,
            _payload: &PaymentPayload,
        ) -> Result<PaymentVerification, PaymentError> {
            unimplemented!()
        }

        async fn settle_payment(
            &self,
            _payload: &PaymentPayload,
        ) -> Result<SettlementResult, PaymentError> {
            unimplemented!()
        }

        async fn get_deposit_wallet_balance(&self) -> Result<u64, PaymentError> {
            Ok(self.balance.load(Ordering::SeqCst))
        }

        async fn transfer_to(
            &self,
            _destination: &str,
            amount: u64,
        ) -> Result<TxResult, PaymentError> {
            if self.transfer_success {
                // Deduct the transferred amount
                self.balance.fetch_sub(amount, Ordering::SeqCst);
                Ok(TxResult {
                    tx_hash: "mock-tx-hash".to_string(),
                    block_number: Some(12345),
                    success: true,
                })
            } else {
                Err(PaymentError::UnsupportedChain("Mock failure".to_string()))
            }
        }

        async fn get_tx_status(&self, _tx_hash: &str) -> Result<TxStatus, PaymentError> {
            Ok(TxStatus::Confirmed { confirmations: 1 })
        }

        async fn health_check(&self) -> Result<bool, PaymentError> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_sweep_above_threshold() {
        let chain: Arc<dyn ChainFacilitator> = Arc::new(MockFacilitator::new(
            Chain::Base,
            20_000_000, // 20 USDC
            true,
        ));

        let operator_addresses = OperatorAddresses {
            base: Some("0xoperator".to_string()),
            near: None,
            solana: None,
        };

        let config = SweepConfig {
            interval: Duration::from_secs(1),
            min_amount_usdc: 10_000_000, // 10 USDC threshold
            reserve_for_gas: 10_000,     // 0.01 USDC reserve
        };

        let sweeper = FundSweeper::new(vec![chain.clone()], operator_addresses, config);

        let records = sweeper.sweep_once().await;

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].chain, Chain::Base);
        assert_eq!(records[0].amount, 20_000_000 - 10_000);
        assert!(records[0].success);
    }

    #[tokio::test]
    async fn test_sweep_below_threshold() {
        let chain: Arc<dyn ChainFacilitator> = Arc::new(MockFacilitator::new(
            Chain::Base,
            5_000_000, // 5 USDC (below threshold)
            true,
        ));

        let operator_addresses = OperatorAddresses {
            base: Some("0xoperator".to_string()),
            near: None,
            solana: None,
        };

        let config = SweepConfig {
            interval: Duration::from_secs(1),
            min_amount_usdc: 10_000_000, // 10 USDC threshold
            reserve_for_gas: 10_000,
        };

        let sweeper = FundSweeper::new(vec![chain], operator_addresses, config);

        let records = sweeper.sweep_once().await;

        assert!(records.is_empty()); // No sweep because below threshold
    }

    #[tokio::test]
    async fn test_sweep_no_operator_address() {
        let chain: Arc<dyn ChainFacilitator> = Arc::new(MockFacilitator::new(
            Chain::Base,
            20_000_000,
            true,
        ));

        let operator_addresses = OperatorAddresses {
            base: None, // No operator configured
            near: None,
            solana: None,
        };

        let config = SweepConfig::default();

        let sweeper = FundSweeper::new(vec![chain], operator_addresses, config);

        let records = sweeper.sweep_once().await;

        assert!(records.is_empty()); // No sweep because no operator
    }
}

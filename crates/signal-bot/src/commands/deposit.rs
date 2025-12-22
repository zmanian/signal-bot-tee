//! Deposit command - shows deposit addresses for adding credits.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;
use tracing::info;
use x402_payments::PaymentConfig;

pub struct DepositHandler {
    config: PaymentConfig,
}

impl DepositHandler {
    pub fn new(config: PaymentConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl CommandHandler for DepositHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!deposit")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        info!("Deposit info requested by {}", message.source);

        let mut sections = Vec::new();

        // Check which chains are enabled and show their deposit info
        if let Some(ref base_config) = self.config.base {
            if base_config.enabled {
                sections.push(format!(
                    "**Base (L2)**\n\
                     Network: Base Mainnet\n\
                     Token: USDC\n\
                     Contract: `{}`\n\
                     Address: _Coming soon_",
                    base_config.usdc_contract
                ));
            }
        }

        if let Some(ref near_config) = self.config.near {
            if near_config.enabled {
                sections.push(format!(
                    "**NEAR Protocol**\n\
                     Token: USDC ({})\n\
                     Account: _Coming soon_\n\
                     Memo: Include your phone number",
                    &near_config.usdc_contract[..12]
                ));
            }
        }

        if let Some(ref solana_config) = self.config.solana {
            if solana_config.enabled {
                sections.push(format!(
                    "**Solana**\n\
                     Token: USDC\n\
                     Mint: `{}`\n\
                     Address: _Coming soon_",
                    solana_config.usdc_mint
                ));
            }
        }

        let response = if sections.is_empty() {
            "**Deposit USDC**\n\n\
             No payment chains are currently configured.\n\
             Please contact the operator.".to_string()
        } else {
            format!(
                "**Deposit USDC**\n\n\
                 Send USDC to one of these addresses:\n\n\
                 {}\n\n\
                 After sending, credits will be added automatically.\n\
                 Use `!balance` to check your balance.",
                sections.join("\n\n")
            )
        };

        Ok(response)
    }
}

//! Balance command - shows user's credit balance.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::info;
use x402_payments::{CreditStore, PricingCalculator};

pub struct BalanceHandler {
    credit_store: Arc<CreditStore>,
}

impl BalanceHandler {
    pub fn new(credit_store: Arc<CreditStore>) -> Self {
        Self { credit_store }
    }
}

#[async_trait]
impl CommandHandler for BalanceHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!balance")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let user_id = &message.source;
        let balance = self.credit_store.get_balance(user_id).await;

        info!("Balance check for {}: {} credits", user_id, balance.credits_remaining);

        let usdc_balance = PricingCalculator::format_usdc(balance.credits_remaining);
        let usdc_deposited = PricingCalculator::format_usdc(balance.total_deposited);
        let usdc_consumed = PricingCalculator::format_usdc(balance.total_consumed);

        let response = if balance.credits_remaining == 0 && balance.total_deposited == 0 {
            format!(
                "**Your Balance**\n\n\
                 You have no credits yet.\n\n\
                 Use `!deposit` to get deposit addresses and add credits."
            )
        } else {
            format!(
                "**Your Balance**\n\n\
                 Credits: {} ({})\n\
                 Total Deposited: {}\n\
                 Total Used: {}\n\n\
                 Use `!deposit` to add more credits.",
                balance.credits_remaining,
                usdc_balance,
                usdc_deposited,
                usdc_consumed,
            )
        };

        Ok(response)
    }
}

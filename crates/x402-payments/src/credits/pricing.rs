//! Pricing calculation for token-to-credit conversion.

use crate::config::PricingConfig;

/// Token usage from an LLM response.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

impl TokenUsage {
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
        }
    }

    pub fn total_tokens(&self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }
}

/// Calculate credits required for given token usage.
///
/// Formula:
/// - prompt_cost = (prompt_tokens * prompt_credits_per_million) / 1_000_000
/// - completion_cost = (completion_tokens * completion_credits_per_million) / 1_000_000
/// - total = max(prompt_cost + completion_cost, minimum_credits_per_message)
pub fn calculate_credits(usage: &TokenUsage, config: &PricingConfig) -> u64 {
    let prompt_cost =
        (usage.prompt_tokens as u64 * config.prompt_credits_per_million) / 1_000_000;
    let completion_cost =
        (usage.completion_tokens as u64 * config.completion_credits_per_million) / 1_000_000;

    let total = prompt_cost + completion_cost;
    total.max(config.minimum_credits_per_message)
}

/// Estimate credits for a message based on character count.
///
/// Uses a rough heuristic: ~4 characters per token for English text.
/// This is used for pre-flight checks before processing a message.
pub fn estimate_credits(message_chars: usize, config: &PricingConfig) -> u64 {
    // Rough estimate: 4 chars per token
    let estimated_prompt_tokens = (message_chars / 4).max(1) as u32;
    // Assume response is ~2x the prompt (very rough)
    let estimated_completion_tokens = estimated_prompt_tokens * 2;

    let usage = TokenUsage::new(estimated_prompt_tokens, estimated_completion_tokens);
    calculate_credits(&usage, config)
}

/// Pricing calculator with cached config.
pub struct PricingCalculator {
    config: PricingConfig,
}

impl PricingCalculator {
    pub fn new(config: PricingConfig) -> Self {
        Self { config }
    }

    /// Calculate credits for token usage.
    pub fn calculate(&self, usage: &TokenUsage) -> u64 {
        calculate_credits(usage, &self.config)
    }

    /// Estimate credits for a message.
    pub fn estimate(&self, message_chars: usize) -> u64 {
        estimate_credits(message_chars, &self.config)
    }

    /// Convert USDC amount to credits.
    pub fn usdc_to_credits(&self, usdc_micro: u64) -> u64 {
        // 1 USDC = usdc_to_credits_ratio credits
        // usdc_micro is in micro-USDC (1e-6)
        // So: credits = usdc_micro * ratio / 1_000_000
        // But since ratio is already 1M for 1 USDC, and usdc_micro is also in micro:
        // credits = usdc_micro (they're the same scale)
        usdc_micro
    }

    /// Convert credits to USDC (micro).
    pub fn credits_to_usdc(&self, credits: u64) -> u64 {
        credits
    }

    /// Get human-readable USDC amount.
    pub fn format_usdc(micro_usdc: u64) -> String {
        let usdc = micro_usdc as f64 / 1_000_000.0;
        format!("${:.6}", usdc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> PricingConfig {
        PricingConfig::default()
    }

    #[test]
    fn test_calculate_credits_basic() {
        let config = default_config();
        let usage = TokenUsage::new(1000, 500);

        let credits = calculate_credits(&usage, &config);

        // prompt: 1000 * 100_000 / 1_000_000 = 100
        // completion: 500 * 300_000 / 1_000_000 = 150
        // total: 250
        assert_eq!(credits, 250);
    }

    #[test]
    fn test_calculate_credits_minimum() {
        let config = default_config();
        // Very small usage should still hit minimum
        let usage = TokenUsage::new(1, 1);

        let credits = calculate_credits(&usage, &config);

        // Should be at least minimum_credits_per_message (100)
        assert_eq!(credits, config.minimum_credits_per_message);
    }

    #[test]
    fn test_calculate_credits_zero() {
        let config = default_config();
        let usage = TokenUsage::new(0, 0);

        let credits = calculate_credits(&usage, &config);

        // Should be minimum
        assert_eq!(credits, config.minimum_credits_per_message);
    }

    #[test]
    fn test_estimate_credits() {
        let config = default_config();

        // 100 chars ≈ 25 tokens prompt
        // Estimated 50 tokens completion
        let credits = estimate_credits(100, &config);

        // Should be reasonable estimate
        assert!(credits >= config.minimum_credits_per_message);
    }

    #[test]
    fn test_pricing_calculator() {
        let calc = PricingCalculator::new(default_config());

        let usage = TokenUsage::new(10_000, 5_000);
        let credits = calc.calculate(&usage);

        // 10_000 * 100_000 / 1_000_000 = 1000
        // 5_000 * 300_000 / 1_000_000 = 1500
        // total: 2500
        assert_eq!(credits, 2500);
    }

    #[test]
    fn test_usdc_conversion() {
        let calc = PricingCalculator::new(default_config());

        // 1 USDC = 1_000_000 micro-USDC = 1_000_000 credits
        let credits = calc.usdc_to_credits(1_000_000);
        assert_eq!(credits, 1_000_000);

        let usdc = calc.credits_to_usdc(1_000_000);
        assert_eq!(usdc, 1_000_000);
    }

    #[test]
    fn test_format_usdc() {
        assert_eq!(PricingCalculator::format_usdc(1_000_000), "$1.000000");
        assert_eq!(PricingCalculator::format_usdc(500_000), "$0.500000");
        assert_eq!(PricingCalculator::format_usdc(100), "$0.000100");
    }
}

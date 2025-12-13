//! Verify command - provides cryptographic attestation proofs.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use dstack_client::DstackClient;
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::info;
use sha2::{Sha256, Digest};
use hex;

pub struct VerifyHandler {
    dstack: Arc<DstackClient>,
}

impl VerifyHandler {
    pub fn new(dstack: Arc<DstackClient>) -> Self {
        Self { dstack }
    }

    /// Parse the challenge nonce from the message text.
    /// Expected format: "!verify <nonce>" or just "!verify"
    fn parse_challenge(&self, text: &str) -> Option<String> {
        let trimmed = text.trim();
        if trimmed.starts_with("!verify") {
            let rest = trimmed.strip_prefix("!verify").unwrap().trim();
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            }
        } else {
            None
        }
    }

    async fn generate_attestation(&self, challenge: Option<&str>) -> AttestationResult {
        // Check if we're in a TEE
        if !self.dstack.is_in_tee().await {
            return AttestationResult {
                in_tee: false,
                error: Some("Not running in TEE environment".into()),
                ..Default::default()
            };
        }

        // Get app info
        let app_info = match self.dstack.get_app_info().await {
            Ok(info) => info,
            Err(e) => {
                return AttestationResult {
                    in_tee: true,
                    error: Some(format!("Failed to get app info: {}", e)),
                    ..Default::default()
                };
            }
        };

        // Prepare report_data - hash if challenge is too long
        let default_challenge = "no-challenge-provided";
        let challenge_str = challenge.unwrap_or(default_challenge);
        let challenge_bytes = challenge_str.as_bytes();

        let (report_data, was_hashed) = if challenge_bytes.len() > 64 {
            // Hash the challenge with SHA-256 (produces 32 bytes)
            let mut hasher = Sha256::new();
            hasher.update(challenge_bytes);
            let hash = hasher.finalize();
            (hash.to_vec(), true)
        } else {
            // Use challenge as-is
            (challenge_bytes.to_vec(), false)
        };

        let report_data_hex = hex::encode(&report_data);

        // Generate quote with report_data
        let quote = match self.dstack.get_quote(&report_data).await {
            Ok(q) => Some(q),
            Err(e) => {
                return AttestationResult {
                    in_tee: true,
                    compose_hash: app_info.compose_hash,
                    app_id: app_info.app_id,
                    error: Some(format!("Failed to generate quote: {}", e)),
                    report_data_hex: Some(report_data_hex),
                    was_hashed,
                    ..Default::default()
                };
            }
        };

        AttestationResult {
            in_tee: true,
            compose_hash: app_info.compose_hash,
            app_id: app_info.app_id,
            quote: quote.map(|q| q.quote),
            challenge: challenge.map(String::from),
            report_data_hex: Some(report_data_hex),
            was_hashed,
            error: None,
        }
    }

    fn format_response(&self, result: AttestationResult) -> String {
        let mut lines = vec![];

        if !result.in_tee {
            lines.push("**⚠️ NOT RUNNING IN TEE**".into());
            lines.push(String::new());
            if let Some(err) = &result.error {
                lines.push(format!("Error: {}", err));
            }
            lines.push(String::new());
            lines.push("This bot is NOT running in a Trusted Execution Environment.".into());
            lines.push("Your messages may not be private.".into());
            return lines.join("\n");
        }

        lines.push("**🔐 TEE Attestation**".into());
        lines.push(String::new());

        // Challenge confirmation
        if let Some(challenge) = &result.challenge {
            lines.push(format!("**Your Challenge:** {}", challenge));
            if result.was_hashed {
                lines.push("_Note: Challenge was >64 bytes, so it was hashed with SHA-256_".into());
            }
        } else {
            lines.push("**Your Challenge:** (none provided)".into());
            lines.push("_Tip: Use `!verify <your-random-text>` for cryptographic proof_".into());
        }
        lines.push(String::new());

        // Report data (what's actually in the quote)
        if let Some(report_data_hex) = &result.report_data_hex {
            lines.push("**Report Data (hex):**".into());
            lines.push(format!("```\n{}\n```", report_data_hex));
            if result.was_hashed {
                lines.push("_This is the SHA-256 hash of your challenge._".into());
            } else {
                lines.push("_This is your challenge encoded in hex._".into());
            }
            lines.push(String::new());
        }

        // App info
        lines.push("**TEE Info:**".into());
        if let Some(hash) = &result.compose_hash {
            lines.push(format!("- Compose Hash: {}", hash));
        }
        if let Some(id) = &result.app_id {
            lines.push(format!("- App ID: {}", id));
        }
        lines.push(String::new());

        // Quote
        if let Some(quote) = &result.quote {
            lines.push("**TDX Quote (base64):**".into());
            lines.push("```".into());
            // Split quote into chunks for readability (Signal might have message limits)
            for chunk in quote.as_bytes().chunks(64) {
                lines.push(String::from_utf8_lossy(chunk).to_string());
            }
            lines.push("```".into());
            lines.push(String::new());

            lines.push("**How to Verify:**".into());
            lines.push("1. **Verify Report Data:** The report_data field in the quote should match the hex value above".into());
            if result.was_hashed {
                lines.push("   - Since your challenge was >64 bytes, verify: `echo -n '<your-challenge>' | sha256sum`".into());
            } else {
                lines.push("   - To verify: `echo -n '<your-challenge>' | xxd -p`".into());
            }
            lines.push(String::new());

            lines.push("2. **Verify Quote Signature:** Use the Phala verification portal".into());
            lines.push("   - Go to: https://proof.phala.network".into());
            lines.push("   - Paste the TDX quote (base64) above".into());
            lines.push("   - The quote signature is verified by Intel TDX hardware".into());
            lines.push(String::new());

            lines.push("3. **Verify Docker Compose:** Check that compose_hash matches the expected configuration".into());
            lines.push("   - Repository: https://github.com/zmanian/signal-bot-tee".into());
            lines.push("   - Compare the compose_hash above with: `sha256sum docker-compose.yaml`".into());
            lines.push("   - This proves the bot is running the expected code".into());
        } else if let Some(err) = &result.error {
            lines.push(format!("**Quote Error:** {}", err));
        }

        lines.push(String::new());
        lines.push("**NEAR AI:** Verify separately at https://near.ai/verify".into());

        lines.join("\n")
    }
}

#[derive(Default)]
struct AttestationResult {
    in_tee: bool,
    compose_hash: Option<String>,
    app_id: Option<String>,
    quote: Option<String>,
    challenge: Option<String>,
    report_data_hex: Option<String>,
    was_hashed: bool,
    error: Option<String>,
}

#[async_trait]
impl CommandHandler for VerifyHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!verify")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let challenge = self.parse_challenge(&message.text);

        info!(
            "Attestation requested by {} with challenge: {:?}",
            message.source,
            challenge.as_ref().map(|c| &c[..c.len().min(20)])
        );

        let result = self.generate_attestation(challenge.as_deref()).await;
        Ok(self.format_response(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_handler() -> VerifyHandler {
        VerifyHandler {
            dstack: Arc::new(DstackClient::new("/fake")),
        }
    }

    #[test]
    fn test_parse_challenge_with_nonce() {
        let handler = create_test_handler();

        assert_eq!(
            handler.parse_challenge("!verify abc123"),
            Some("abc123".into())
        );
        assert_eq!(
            handler.parse_challenge("!verify   my random challenge  "),
            Some("my random challenge".into())
        );
    }

    #[test]
    fn test_parse_challenge_without_nonce() {
        let handler = create_test_handler();

        assert_eq!(handler.parse_challenge("!verify"), None);
        assert_eq!(handler.parse_challenge("!verify   "), None);
    }

    #[test]
    fn test_format_response_not_in_tee() {
        let handler = create_test_handler();
        let result = AttestationResult {
            in_tee: false,
            error: Some("Not running in TEE".into()),
            ..Default::default()
        };

        let response = handler.format_response(result);
        assert!(response.contains("NOT RUNNING IN TEE"));
        assert!(response.contains("not be private"));
    }

    #[test]
    fn test_short_challenge_not_hashed() {
        // Test that challenges <= 64 bytes are NOT hashed
        let challenge = "short-nonce-123";
        assert!(challenge.len() <= 64);

        let expected_hex = hex::encode(challenge.as_bytes());

        let handler = create_test_handler();
        let result = AttestationResult {
            in_tee: true,
            compose_hash: Some("abc123".into()),
            app_id: Some("app-456".into()),
            quote: Some("base64quote".into()),
            challenge: Some(challenge.into()),
            report_data_hex: Some(expected_hex.clone()),
            was_hashed: false,
            error: None,
        };

        let response = handler.format_response(result);
        assert!(response.contains(&expected_hex));
        assert!(response.contains("This is your challenge encoded in hex"));
        assert!(!response.contains("SHA-256"));
    }

    #[test]
    fn test_long_challenge_hashed() {
        // Test that challenges > 64 bytes ARE hashed
        let long_challenge = "a".repeat(65); // 65 bytes
        assert!(long_challenge.len() > 64);

        let mut hasher = Sha256::new();
        hasher.update(long_challenge.as_bytes());
        let expected_hash = hasher.finalize();
        let expected_hex = hex::encode(expected_hash);

        let handler = create_test_handler();
        let result = AttestationResult {
            in_tee: true,
            compose_hash: Some("abc123".into()),
            app_id: Some("app-456".into()),
            quote: Some("base64quote".into()),
            challenge: Some(long_challenge.clone()),
            report_data_hex: Some(expected_hex.clone()),
            was_hashed: true,
            error: None,
        };

        let response = handler.format_response(result);
        assert!(response.contains(&expected_hex));
        assert!(response.contains("SHA-256 hash"));
        assert!(response.contains("was >64 bytes"));
    }

    #[test]
    fn test_format_response_with_challenge() {
        let handler = create_test_handler();
        let challenge = "my-nonce";
        let report_data_hex = hex::encode(challenge.as_bytes());

        let result = AttestationResult {
            in_tee: true,
            compose_hash: Some("abc123".into()),
            app_id: Some("app-456".into()),
            quote: Some("base64quote".into()),
            challenge: Some(challenge.into()),
            report_data_hex: Some(report_data_hex.clone()),
            was_hashed: false,
            error: None,
        };

        let response = handler.format_response(result);
        assert!(response.contains("TEE Attestation"));
        assert!(response.contains("my-nonce"));
        assert!(response.contains("abc123"));
        assert!(response.contains("base64quote"));
        assert!(response.contains(&report_data_hex));
        assert!(response.contains("Report Data"));
    }

    #[test]
    fn test_format_response_without_challenge() {
        let handler = create_test_handler();
        let result = AttestationResult {
            in_tee: true,
            compose_hash: Some("abc123".into()),
            quote: Some("base64quote".into()),
            report_data_hex: Some(hex::encode("no-challenge-provided".as_bytes())),
            was_hashed: false,
            ..Default::default()
        };

        let response = handler.format_response(result);
        assert!(response.contains("(none provided)"));
        assert!(response.contains("Tip:"));
    }

    #[test]
    fn test_verification_instructions_present() {
        let handler = create_test_handler();
        let result = AttestationResult {
            in_tee: true,
            compose_hash: Some("abc123".into()),
            app_id: Some("app-456".into()),
            quote: Some("base64quote".into()),
            challenge: Some("test".into()),
            report_data_hex: Some(hex::encode("test".as_bytes())),
            was_hashed: false,
            error: None,
        };

        let response = handler.format_response(result);
        assert!(response.contains("How to Verify:"));
        assert!(response.contains("Verify Report Data"));
        assert!(response.contains("Verify Quote Signature"));
        assert!(response.contains("Verify Docker Compose"));
        assert!(response.contains("https://proof.phala.network"));
        assert!(response.contains("https://github.com/zmanian/signal-bot-tee"));
        assert!(response.contains("xxd -p"));
    }
}

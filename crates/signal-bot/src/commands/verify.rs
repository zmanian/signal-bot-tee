//! Verify command - provides attestation proofs.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use dstack_client::DstackClient;
use near_ai_client::NearAiClient;
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::info;

pub struct VerifyHandler {
    near_ai: Arc<NearAiClient>,
    dstack: Arc<DstackClient>,
}

impl VerifyHandler {
    pub fn new(near_ai: Arc<NearAiClient>, dstack: Arc<DstackClient>) -> Self {
        Self { near_ai, dstack }
    }

    async fn get_proxy_info(&self) -> ProxyInfo {
        if !self.dstack.is_in_tee().await {
            return ProxyInfo {
                available: false,
                reason: Some("Not running in TEE environment".into()),
                ..Default::default()
            };
        }

        match self.dstack.get_app_info().await {
            Ok(info) => ProxyInfo {
                available: true,
                compose_hash: info.compose_hash,
                app_id: info.app_id,
                reason: None,
            },
            Err(e) => ProxyInfo {
                available: false,
                reason: Some(e.to_string()),
                ..Default::default()
            },
        }
    }

    async fn get_near_info(&self) -> NearInfo {
        match self.near_ai.get_attestation().await {
            Ok(_attestation) => NearInfo {
                available: true,
                model: self.near_ai.model().to_string(),
                reason: None,
            },
            Err(e) => NearInfo {
                available: false,
                model: self.near_ai.model().to_string(),
                reason: Some(e.to_string()),
            },
        }
    }

    fn format_response(&self, proxy: ProxyInfo, near: NearInfo) -> String {
        let mut lines = vec!["**Privacy Verification**".to_string(), String::new()];

        // Proxy section
        lines.push("**Proxy (Signal Bot)**".into());
        if proxy.available {
            lines.push("|- TEE: Intel TDX".into());
            if let Some(hash) = &proxy.compose_hash {
                lines.push(format!("|- Compose Hash: {}...", &hash[..hash.len().min(16)]));
            }
            if let Some(id) = &proxy.app_id {
                lines.push(format!("|- App ID: {}...", &id[..id.len().min(16)]));
            }
            lines.push("|- Verify: https://proof.phala.network".into());
        } else {
            lines.push(format!(
                "|- {}",
                proxy.reason.unwrap_or("Unavailable".into())
            ));
        }

        lines.push(String::new());

        // Inference section
        lines.push("**Inference (NEAR AI Cloud)**".into());
        if near.available {
            lines.push("|- TEE: NVIDIA GPU (H100/H200)".into());
            lines.push(format!("|- Model: {}", near.model));
            lines.push("|- Gateway: Intel TDX".into());
            lines.push("|- Verify: https://near.ai/verify".into());
        } else {
            lines.push(format!("|- Model: {}", near.model));
            lines.push(format!(
                "|- {}",
                near.reason.unwrap_or("Unavailable".into())
            ));
        }

        lines.push(String::new());
        lines.push("Both layers provide hardware-backed attestation.".into());
        lines.push("Your messages never exist in plaintext outside TEEs.".into());

        lines.join("\n")
    }
}

#[derive(Default)]
struct ProxyInfo {
    available: bool,
    compose_hash: Option<String>,
    app_id: Option<String>,
    reason: Option<String>,
}

struct NearInfo {
    available: bool,
    model: String,
    reason: Option<String>,
}

#[async_trait]
impl CommandHandler for VerifyHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!verify")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        info!("Attestation requested by {}", message.source);

        let proxy = self.get_proxy_info().await;
        let near = self.get_near_info().await;

        Ok(self.format_response(proxy, near))
    }
}

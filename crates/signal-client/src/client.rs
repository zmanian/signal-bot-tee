//! Signal HTTP client.

use crate::error::SignalError;
use crate::types::*;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, instrument, warn};
use urlencoding::encode;

/// Signal CLI REST API client.
#[derive(Clone)]
pub struct SignalClient {
    client: Client,
    base_url: String,
    phone_number: String,
}

impl SignalClient {
    /// Create a new Signal client.
    pub fn new(
        base_url: impl Into<String>,
        phone_number: impl Into<String>,
    ) -> Result<Self, SignalError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            phone_number: phone_number.into(),
        })
    }

    /// Get the configured phone number.
    pub fn phone_number(&self) -> &str {
        &self.phone_number
    }

    /// Check if the Signal API is healthy.
    pub async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/v1/health", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Get account information.
    #[instrument(skip(self))]
    pub async fn get_account(&self) -> Result<Account, SignalError> {
        let encoded_number = encode(&self.phone_number);
        let response = self
            .client
            .get(format!("{}/v1/accounts/{}", self.base_url, encoded_number))
            .send()
            .await?;

        if !response.status().is_success() {
            let msg = response.text().await.unwrap_or_default();
            return Err(SignalError::Api(msg));
        }

        Ok(response.json().await?)
    }

    /// Receive pending messages.
    #[instrument(skip(self))]
    pub async fn receive(&self) -> Result<Vec<IncomingMessage>, SignalError> {
        let encoded_number = encode(&self.phone_number);
        let response = self
            .client
            .get(format!(
                "{}/v1/receive/{}",
                self.base_url, encoded_number
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            let msg = response.text().await.unwrap_or_default();
            return Err(SignalError::Api(msg));
        }

        let messages: Vec<IncomingMessage> = response.json().await?;
        debug!("Received {} messages", messages.len());
        Ok(messages)
    }

    /// Send a message to a recipient.
    #[instrument(skip(self, message))]
    pub async fn send(&self, recipient: &str, message: &str) -> Result<(), SignalError> {
        let request = SendMessageRequest {
            message: message.to_string(),
            number: Some(self.phone_number.clone()),
            recipients: Some(vec![recipient.to_string()]),
        };

        let response = self
            .client
            .post(format!("{}/v2/send", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let msg = response.text().await.unwrap_or_default();
            warn!("Send failed: {}", msg);
            return Err(SignalError::SendFailed(msg));
        }

        debug!("Sent message to {}", recipient);
        Ok(())
    }

    /// Reply to a message (handles both direct and group messages).
    pub async fn reply(&self, original: &BotMessage, message: &str) -> Result<(), SignalError> {
        self.send(original.reply_target(), message).await
    }
}

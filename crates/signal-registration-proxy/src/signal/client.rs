//! Signal CLI REST API client for registration operations.

use crate::error::ProxyError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, instrument, warn};
use urlencoding::encode;

/// Signal CLI REST API client focused on registration operations.
#[derive(Clone)]
pub struct SignalRegistrationClient {
    client: Client,
    base_url: String,
}

impl SignalRegistrationClient {
    /// Create a new Signal registration client.
    pub fn new(base_url: impl Into<String>) -> Result<Self, ProxyError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ProxyError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            base_url: base_url.into(),
        })
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

    /// Initiate registration for a phone number.
    ///
    /// This triggers Signal to send a verification code via SMS or voice call.
    #[instrument(skip(self, captcha))]
    pub async fn register(
        &self,
        phone_number: &str,
        captcha: Option<&str>,
        use_voice: bool,
    ) -> Result<(), ProxyError> {
        let encoded_number = encode(phone_number);

        let mut url = format!("{}/v1/register/{}", self.base_url, encoded_number);

        // Add query parameters
        let mut params = Vec::new();
        if use_voice {
            params.push("voice=true".to_string());
        }
        if let Some(token) = captcha {
            params.push(format!("captcha={}", encode(token)));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        debug!(url = %url, "Sending registration request");

        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Signal registration failed");

            // Parse specific error types
            if body.contains("captcha") {
                return Err(ProxyError::SignalApi(
                    "CAPTCHA required. Please provide a captcha token.".to_string(),
                ));
            }

            return Err(ProxyError::SignalApi(format!(
                "Registration failed: {} - {}",
                status, body
            )));
        }

        debug!(phone_number = %phone_number, "Registration request sent successfully");
        Ok(())
    }

    /// Verify registration with the code received via SMS/voice.
    #[instrument(skip(self, code, pin))]
    pub async fn verify(
        &self,
        phone_number: &str,
        code: &str,
        pin: Option<&str>,
    ) -> Result<(), ProxyError> {
        let encoded_number = encode(phone_number);
        let encoded_code = encode(code);

        let mut url = format!(
            "{}/v1/register/{}/verify/{}",
            self.base_url, encoded_number, encoded_code
        );

        if let Some(p) = pin {
            url = format!("{}?pin={}", url, encode(p));
        }

        debug!(url = %url, "Sending verification request");

        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Signal verification failed");

            if body.contains("Invalid verification code") || body.contains("incorrect") {
                return Err(ProxyError::SignalApi("Invalid verification code".to_string()));
            }

            return Err(ProxyError::SignalApi(format!(
                "Verification failed: {} - {}",
                status, body
            )));
        }

        debug!(phone_number = %phone_number, "Verification successful");
        Ok(())
    }

    /// Unregister a phone number from Signal.
    #[instrument(skip(self))]
    pub async fn unregister(&self, phone_number: &str) -> Result<(), ProxyError> {
        let encoded_number = encode(phone_number);
        let url = format!("{}/v1/unregister/{}", self.base_url, encoded_number);

        debug!(url = %url, "Sending unregister request");

        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Signal unregister failed");

            return Err(ProxyError::SignalApi(format!(
                "Unregister failed: {} - {}",
                status, body
            )));
        }

        debug!(phone_number = %phone_number, "Unregister successful");
        Ok(())
    }

    /// Get account information for a registered number.
    #[instrument(skip(self))]
    pub async fn get_account(&self, phone_number: &str) -> Result<AccountInfo, ProxyError> {
        let encoded_number = encode(phone_number);
        let url = format!("{}/v1/accounts/{}", self.base_url, encoded_number);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProxyError::SignalApi(format!(
                "Get account failed: {} - {}",
                status, body
            )));
        }

        let account: AccountInfo = response.json().await.map_err(|e| {
            ProxyError::SignalApi(format!("Failed to parse account response: {}", e))
        })?;

        Ok(account)
    }

    /// List all registered accounts.
    #[instrument(skip(self))]
    pub async fn list_accounts(&self) -> Result<Vec<String>, ProxyError> {
        let url = format!("{}/v1/accounts", self.base_url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProxyError::SignalApi(format!(
                "List accounts failed: {} - {}",
                status, body
            )));
        }

        let accounts: Vec<String> = response.json().await.map_err(|e| {
            ProxyError::SignalApi(format!("Failed to parse accounts response: {}", e))
        })?;

        Ok(accounts)
    }
}

/// Account information from Signal CLI API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub number: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = SignalRegistrationClient::new("http://localhost:8080");
        assert!(client.is_ok());
    }
}

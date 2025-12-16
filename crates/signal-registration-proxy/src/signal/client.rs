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
        let url = format!("{}/v1/register/{}", self.base_url, encoded_number);

        // Build JSON body with captcha and use_voice
        let body = RegisterRequestBody {
            captcha: captcha.map(String::from),
            use_voice: Some(use_voice),
        };

        debug!(url = %url, use_voice = %use_voice, has_captcha = %captcha.is_some(), "Sending registration request");

        let response = self.client.post(&url).json(&body).send().await?;

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

    /// Update profile for a phone number.
    #[instrument(skip(self))]
    pub async fn update_profile(
        &self,
        phone_number: &str,
        name: Option<&str>,
        about: Option<&str>,
    ) -> Result<(), ProxyError> {
        let encoded_number = encode(phone_number);
        let url = format!("{}/v1/profiles/{}", self.base_url, encoded_number);

        let body = ProfileRequestBody {
            name: name.map(String::from),
            about: about.map(String::from),
        };

        debug!(url = %url, "Sending profile update request");

        let response = self.client.put(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Signal profile update failed");

            return Err(ProxyError::SignalApi(format!(
                "Profile update failed: {} - {}",
                status, body
            )));
        }

        debug!(phone_number = %phone_number, "Profile updated successfully");
        Ok(())
    }

    /// Set username for a phone number.
    #[instrument(skip(self))]
    pub async fn set_username(
        &self,
        phone_number: &str,
        username: &str,
    ) -> Result<UsernameInfo, ProxyError> {
        let encoded_number = encode(phone_number);
        let url = format!("{}/v1/accounts/{}/username", self.base_url, encoded_number);

        let body = UsernameRequestBody {
            username: username.to_string(),
        };

        debug!(url = %url, username = %username, "Sending set username request");

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Signal set username failed");

            return Err(ProxyError::SignalApi(format!(
                "Set username failed: {} - {}",
                status, body
            )));
        }

        let info: UsernameInfo = response.json().await.map_err(|e| {
            ProxyError::SignalApi(format!("Failed to parse username response: {}", e))
        })?;

        debug!(phone_number = %phone_number, username = ?info.username, "Username set successfully");
        Ok(info)
    }

    /// Delete username for a phone number.
    #[instrument(skip(self))]
    pub async fn delete_username(&self, phone_number: &str) -> Result<(), ProxyError> {
        let encoded_number = encode(phone_number);
        let url = format!("{}/v1/accounts/{}/username", self.base_url, encoded_number);

        debug!(url = %url, "Sending delete username request");

        let response = self.client.delete(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Signal delete username failed");

            return Err(ProxyError::SignalApi(format!(
                "Delete username failed: {} - {}",
                status, body
            )));
        }

        debug!(phone_number = %phone_number, "Username deleted successfully");
        Ok(())
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

/// Request body for registration endpoint.
#[derive(Debug, Clone, Serialize)]
struct RegisterRequestBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    captcha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    use_voice: Option<bool>,
}

/// Request body for profile update.
#[derive(Debug, Clone, Serialize)]
struct ProfileRequestBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about: Option<String>,
}

/// Request body for username.
#[derive(Debug, Clone, Serialize)]
struct UsernameRequestBody {
    username: String,
}

/// Response from username endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct UsernameInfo {
    pub username: Option<String>,
    pub username_link: Option<String>,
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

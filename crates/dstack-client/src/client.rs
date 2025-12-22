//! Dstack guest agent client implementation.

use crate::error::DstackError;
use crate::types::*;
use hyper::{Body, Client, Method, Request};
use hyperlocal::{UnixClientExt, Uri};
use std::path::Path;
use tracing::{debug, instrument, warn};

/// Client for Dstack guest agent.
#[derive(Clone, Debug)]
pub struct DstackClient {
    socket_path: String,
}

impl DstackClient {
    /// Create a new Dstack client.
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    /// Check if running inside a TEE.
    pub async fn is_in_tee(&self) -> bool {
        if !Path::new(&self.socket_path).exists() {
            return false;
        }
        self.get_app_info().await.is_ok()
    }

    /// Get application information.
    #[instrument(skip(self))]
    pub async fn get_app_info(&self) -> Result<AppInfo, DstackError> {
        let response = self.request(Method::GET, "/Info", None).await?;
        let info: AppInfo = serde_json::from_slice(&response)?;
        debug!("Got app info: {:?}", info);
        Ok(info)
    }

    /// Generate TDX attestation quote.
    #[instrument(skip(self, report_data))]
    pub async fn get_quote(&self, report_data: &[u8]) -> Result<Quote, DstackError> {
        // Pad or truncate report_data to 64 bytes
        let mut data = [0u8; 64];
        let len = report_data.len().min(64);
        data[..len].copy_from_slice(&report_data[..len]);

        let hex_data = hex::encode(data);
        let path = format!("/GetQuote?report_data={}", hex_data);

        let response = self.request(Method::GET, &path, None).await?;
        let quote: Quote = serde_json::from_slice(&response)?;

        debug!("Generated quote with {} bytes", quote.quote.len());
        Ok(quote)
    }

    /// Derive a key from TEE root of trust.
    #[instrument(skip(self))]
    pub async fn derive_key(
        &self,
        path: &str,
        subject: Option<&str>,
    ) -> Result<Vec<u8>, DstackError> {
        let request = DeriveKeyRequest {
            path: path.to_string(),
            subject: subject.map(String::from),
        };

        let body = serde_json::to_vec(&request)?;
        let response = self.request(Method::POST, "/DeriveKey", Some(body)).await?;
        let result: DeriveKeyResponse = serde_json::from_slice(&response)?;

        hex::decode(&result.key).map_err(|e| DstackError::KeyDerivation(e.to_string()))
    }

    /// Get RA-TLS certificate.
    #[instrument(skip(self))]
    pub async fn get_ra_tls_cert(&self) -> Result<Vec<u8>, DstackError> {
        let response = self.request(Method::GET, "/GetRaTlsCert", None).await?;
        let cert: RaTlsCert = serde_json::from_slice(&response)?;

        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD
            .decode(&cert.cert)
            .map_err(|e| DstackError::QuoteGeneration(e.to_string()))
    }

    /// Make HTTP request to Dstack socket.
    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, DstackError> {
        if !Path::new(&self.socket_path).exists() {
            return Err(DstackError::SocketNotFound(self.socket_path.clone()));
        }

        let client = Client::unix();
        let uri = Uri::new(&self.socket_path, path);

        let body = match body {
            Some(b) => Body::from(b),
            None => Body::empty(),
        };

        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| {
                DstackError::QuoteGeneration(format!("Failed to build request: {}", e))
            })?;

        let response = client.request(request).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = hyper::body::to_bytes(response.into_body()).await?;
            let msg = String::from_utf8_lossy(&body);
            warn!("Dstack request failed: {} - {}", status, msg);
            return Err(DstackError::QuoteGeneration(format!(
                "HTTP {}: {}",
                status, msg
            )));
        }

        let body = hyper::body::to_bytes(response.into_body()).await?;
        Ok(body.to_vec())
    }
}

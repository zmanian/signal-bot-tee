//! Common test utilities for integration tests.

use near_ai_client::NearAiClient;
use std::time::Duration;
use wiremock::MockServer;

/// Start a mock NEAR AI server.
pub async fn mock_near_ai_server() -> MockServer {
    MockServer::start().await
}

/// Create a NEAR AI client configured for a mock server.
pub fn test_near_ai_client(mock_server: &MockServer) -> NearAiClient {
    NearAiClient::new(
        "test-api-key",
        mock_server.uri(),
        "test-model",
        Duration::from_secs(5),
    )
    .unwrap()
}

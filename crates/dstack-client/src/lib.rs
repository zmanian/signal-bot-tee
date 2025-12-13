//! Dstack TEE guest agent client.

mod client;
mod error;
mod types;

pub use client::DstackClient;
pub use error::DstackError;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_in_tee_when_socket_not_exists() {
        let client = DstackClient::new("/nonexistent/socket/path");
        assert!(!client.is_in_tee().await);
    }

    #[tokio::test]
    async fn test_get_app_info_when_socket_not_exists() {
        let client = DstackClient::new("/nonexistent/socket/path");
        let result = client.get_app_info().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DstackError::SocketNotFound(_))));
    }

    #[tokio::test]
    async fn test_get_quote_when_socket_not_exists() {
        let client = DstackClient::new("/nonexistent/socket/path");
        let result = client.get_quote(b"test data").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DstackError::SocketNotFound(_))));
    }

    #[tokio::test]
    async fn test_derive_key_when_socket_not_exists() {
        let client = DstackClient::new("/nonexistent/socket/path");
        let result = client.derive_key("/test/path", None).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DstackError::SocketNotFound(_))));
    }

    #[tokio::test]
    async fn test_get_ra_tls_cert_when_socket_not_exists() {
        let client = DstackClient::new("/nonexistent/socket/path");
        let result = client.get_ra_tls_cert().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DstackError::SocketNotFound(_))));
    }

    #[test]
    fn test_app_info_deserialization() {
        let json = r#"{
            "app_id": "test-app",
            "compose_hash": "abc123",
            "instance_id": "instance-1",
            "custom_field": "extra"
        }"#;

        let info: AppInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.app_id, Some("test-app".into()));
        assert_eq!(info.compose_hash, Some("abc123".into()));
        assert_eq!(info.instance_id, Some("instance-1".into()));
    }

    #[test]
    fn test_app_info_with_missing_fields() {
        let json = r#"{}"#;

        let info: AppInfo = serde_json::from_str(json).unwrap();
        assert!(info.app_id.is_none());
        assert!(info.compose_hash.is_none());
        assert!(info.instance_id.is_none());
    }

    #[test]
    fn test_quote_deserialization() {
        let json = r#"{
            "quote": "base64encodedquote",
            "report_data": "hexdata"
        }"#;

        let quote: Quote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.quote, "base64encodedquote");
        assert_eq!(quote.report_data, Some("hexdata".into()));
    }

    #[test]
    fn test_derive_key_request_serialization() {
        let request = DeriveKeyRequest {
            path: "/test/path".into(),
            subject: Some("test-subject".into()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"path\":\"/test/path\""));
        assert!(json.contains("\"subject\":\"test-subject\""));
    }

    #[test]
    fn test_derive_key_request_without_subject() {
        let request = DeriveKeyRequest {
            path: "/test/path".into(),
            subject: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"path\":\"/test/path\""));
        assert!(!json.contains("subject"));
    }

    #[test]
    fn test_derive_key_response_deserialization() {
        let json = r#"{"key": "deadbeef"}"#;

        let response: DeriveKeyResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.key, "deadbeef");
    }

    #[test]
    fn test_ra_tls_cert_deserialization() {
        let json = r#"{"cert": "Y2VydGlmaWNhdGU="}"#;

        let cert: RaTlsCert = serde_json::from_str(json).unwrap();
        assert_eq!(cert.cert, "Y2VydGlmaWNhdGU=");
    }
}

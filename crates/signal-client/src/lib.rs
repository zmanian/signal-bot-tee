//! Signal CLI REST API client.

mod client;
mod error;
mod receiver;
mod types;

pub use client::SignalClient;
pub use error::SignalError;
pub use receiver::MessageReceiver;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_test_client(mock_server: &MockServer) -> SignalClient {
        SignalClient::new(mock_server.uri()).unwrap()
    }

    #[tokio::test]
    async fn test_health_check_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        assert!(client.health_check().await);
    }

    #[tokio::test]
    async fn test_health_check_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/health"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        assert!(!client.health_check().await);
    }

    #[tokio::test]
    async fn test_list_accounts() {
        let mock_server = MockServer::start().await;

        let accounts = serde_json::json!(["+15555555555", "+16666666666"]);

        Mock::given(method("GET"))
            .and(path("/v1/accounts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&accounts))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.list_accounts().await;

        assert!(result.is_ok());
        let accs = result.unwrap();
        assert_eq!(accs.len(), 2);
        assert_eq!(accs[0], "+15555555555");
    }

    #[tokio::test]
    async fn test_receive_messages() {
        let mock_server = MockServer::start().await;

        let messages = serde_json::json!([
            {
                "envelope": {
                    "source": "+14155551234",
                    "sourceNumber": "+14155551234",
                    "sourceName": "Test User",
                    "timestamp": 1677652288000i64,
                    "dataMessage": {
                        "message": "Hello bot!",
                        "timestamp": 1677652288000i64,
                        "groupInfo": null
                    }
                },
                "account": "+15555555555"
            }
        ]);

        // Note: + is URL-encoded as %2B
        Mock::given(method("GET"))
            .and(path("/v1/receive/%2B15555555555"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&messages))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.receive("+15555555555").await;

        assert!(result.is_ok());
        let msgs = result.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].envelope.source, "+14155551234");
    }

    #[tokio::test]
    async fn test_send_message() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v2/send"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "timestamp": 1677652288000i64
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.send("+15555555555", "+14155551234", "Hello!").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_message_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v2/send"))
            .respond_with(ResponseTemplate::new(400).set_body_string("Invalid recipient"))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.send("+15555555555", "+14155551234", "Hello!").await;

        assert!(result.is_err());
        assert!(matches!(result, Err(SignalError::SendFailed(_))));
    }

    #[tokio::test]
    async fn test_get_account() {
        let mock_server = MockServer::start().await;

        let account = serde_json::json!({
            "number": "+15555555555",
            "uuid": "test-uuid",
            "registered": true
        });

        // Note: + is URL-encoded as %2B
        Mock::given(method("GET"))
            .and(path("/v1/accounts/%2B15555555555"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&account))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.get_account("+15555555555").await;

        assert!(result.is_ok());
        let acc = result.unwrap();
        assert_eq!(acc.number, "+15555555555");
        assert!(acc.registered);
    }

    #[tokio::test]
    async fn test_bot_message_from_incoming() {
        let incoming = IncomingMessage {
            envelope: Envelope {
                source: "+14155551234".into(),
                source_number: Some("+14155551234".into()),
                source_name: Some("Test User".into()),
                timestamp: 1677652288000,
                data_message: Some(DataMessage {
                    message: Some("Hello bot!".into()),
                    timestamp: 1677652288000,
                    group_info: None,
                }),
            },
            account: "+15555555555".into(),
        };

        let bot_msg = BotMessage::from_incoming(&incoming);
        assert!(bot_msg.is_some());

        let msg = bot_msg.unwrap();
        assert_eq!(msg.source, "+14155551234");
        assert_eq!(msg.text, "Hello bot!");
        assert_eq!(msg.receiving_account, "+15555555555");
        assert!(!msg.is_group);
        assert!(msg.group_id.is_none());
    }

    #[tokio::test]
    async fn test_bot_message_from_group() {
        let incoming = IncomingMessage {
            envelope: Envelope {
                source: "+14155551234".into(),
                source_number: Some("+14155551234".into()),
                source_name: Some("Test User".into()),
                timestamp: 1677652288000,
                data_message: Some(DataMessage {
                    message: Some("Hello group!".into()),
                    timestamp: 1677652288000,
                    group_info: Some(GroupInfo {
                        group_id: "test-group-id".into(),
                    }),
                }),
            },
            account: "+15555555555".into(),
        };

        let bot_msg = BotMessage::from_incoming(&incoming);
        assert!(bot_msg.is_some());

        let msg = bot_msg.unwrap();
        assert!(msg.is_group);
        assert_eq!(msg.group_id, Some("test-group-id".into()));
        assert_eq!(msg.reply_target(), "test-group-id");
        assert_eq!(msg.receiving_account, "+15555555555");
    }

    #[tokio::test]
    async fn test_bot_message_no_data_message() {
        let incoming = IncomingMessage {
            envelope: Envelope {
                source: "+14155551234".into(),
                source_number: None,
                source_name: None,
                timestamp: 1677652288000,
                data_message: None,
            },
            account: "+15555555555".into(),
        };

        let bot_msg = BotMessage::from_incoming(&incoming);
        assert!(bot_msg.is_none());
    }
}

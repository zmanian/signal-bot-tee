//! In-memory conversation storage for TEE environments.
//!
//! All conversation data is kept in TEE-protected memory with
//! automatic TTL-based expiration. No external persistence.

mod error;
mod store;
mod types;

pub use error::ConversationError;
pub use store::ConversationStore;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_stored_message_new() {
        let msg = StoredMessage::new("user", "Hello, world!");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, Some("Hello, world!".into()));
    }

    #[test]
    fn test_stored_message_serialization() {
        let msg = StoredMessage::new("assistant", "Hi there!");
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"content\":\"Hi there!\""));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn test_stored_message_deserialization() {
        let json = r#"{
            "role": "user",
            "content": "Test message",
            "timestamp": "2024-01-01T00:00:00Z"
        }"#;

        let msg: StoredMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, Some("Test message".into()));
    }

    #[test]
    fn test_conversation_new() {
        let conv = Conversation::new("user123", Some("You are a helpful assistant".into()));

        assert_eq!(conv.user_id, "user123");
        assert!(conv.messages.is_empty());
        assert_eq!(conv.system_prompt, Some("You are a helpful assistant".into()));
    }

    #[test]
    fn test_conversation_new_without_system_prompt() {
        let conv = Conversation::new("user456", None);

        assert_eq!(conv.user_id, "user456");
        assert!(conv.system_prompt.is_none());
    }

    #[test]
    fn test_conversation_add_message() {
        let mut conv = Conversation::new("user123", None);

        conv.add_message("user", "Hello");
        conv.add_message("assistant", "Hi!");

        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].role, "user");
        assert_eq!(conv.messages[0].content, Some("Hello".into()));
        assert_eq!(conv.messages[1].role, "assistant");
        assert_eq!(conv.messages[1].content, Some("Hi!".into()));
    }

    #[test]
    fn test_conversation_trim_when_under_limit() {
        let mut conv = Conversation::new("user123", None);
        conv.add_message("user", "Message 1");
        conv.add_message("assistant", "Reply 1");

        conv.trim(10);

        assert_eq!(conv.messages.len(), 2);
    }

    #[test]
    fn test_conversation_trim_when_over_limit() {
        let mut conv = Conversation::new("user123", None);
        for i in 1..=10 {
            conv.add_message("user", &format!("Message {}", i));
        }

        assert_eq!(conv.messages.len(), 10);

        conv.trim(5);

        assert_eq!(conv.messages.len(), 5);
        // Should keep the most recent messages (6-10)
        assert_eq!(conv.messages[0].content, Some("Message 6".into()));
        assert_eq!(conv.messages[4].content, Some("Message 10".into()));
    }

    #[test]
    fn test_conversation_trim_to_exact_limit() {
        let mut conv = Conversation::new("user123", None);
        for i in 1..=5 {
            conv.add_message("user", &format!("Message {}", i));
        }

        conv.trim(5);

        assert_eq!(conv.messages.len(), 5);
        assert_eq!(conv.messages[0].content, Some("Message 1".into()));
    }

    #[test]
    fn test_conversation_serialization() {
        let mut conv = Conversation::new("user123", Some("System prompt".into()));
        conv.add_message("user", "Hello");

        let json = serde_json::to_string(&conv).unwrap();

        assert!(json.contains("\"user_id\":\"user123\""));
        assert!(json.contains("\"system_prompt\":\"System prompt\""));
        assert!(json.contains("\"messages\""));
    }

    #[test]
    fn test_conversation_deserialization() {
        let json = r#"{
            "user_id": "user123",
            "messages": [
                {"role": "user", "content": "Hello", "timestamp": "2024-01-01T00:00:00Z"}
            ],
            "system_prompt": "Be helpful",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;

        let conv: Conversation = serde_json::from_str(json).unwrap();
        assert_eq!(conv.user_id, "user123");
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.system_prompt, Some("Be helpful".into()));
    }

    #[test]
    fn test_openai_message_serialization() {
        let msg = OpenAiMessage {
            role: "system".into(),
            content: Some("You are a helpful assistant".into()),
            tool_calls: None,
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"system\""));
        assert!(json.contains("\"content\":\"You are a helpful assistant\""));
    }

    #[test]
    fn test_conversation_updated_at_changes() {
        let mut conv = Conversation::new("user123", None);
        let initial_updated = conv.updated_at;

        // Sleep briefly to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));

        conv.add_message("user", "Hello");

        assert!(conv.updated_at > initial_updated);
    }

    // In-memory store tests

    #[tokio::test]
    async fn test_store_add_and_get_message() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        store.add_message("user1", "user", "Hello", None).await.unwrap();

        let conv = store.get("user1").await.unwrap();
        assert!(conv.is_some());
        let conv = conv.unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].content, Some("Hello".into()));
    }

    #[tokio::test]
    async fn test_store_multiple_messages() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        store.add_message("user1", "user", "Hi", None).await.unwrap();
        store.add_message("user1", "assistant", "Hello!", None).await.unwrap();
        store.add_message("user1", "user", "How are you?", None).await.unwrap();

        let count = store.message_count("user1").await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_store_clear_conversation() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        store.add_message("user1", "user", "Hello", None).await.unwrap();
        assert!(store.get("user1").await.unwrap().is_some());

        let cleared = store.clear("user1").await.unwrap();
        assert!(cleared);

        assert!(store.get("user1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_store_clear_nonexistent() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        let cleared = store.clear("nonexistent").await.unwrap();
        assert!(!cleared);
    }

    #[tokio::test]
    async fn test_store_to_openai_messages() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        store.add_message("user1", "user", "Hello", Some("Be helpful")).await.unwrap();
        store.add_message("user1", "assistant", "Hi there!", None).await.unwrap();

        let messages = store.to_openai_messages("user1", Some("Be helpful")).await.unwrap();

        assert_eq!(messages.len(), 3); // system + 2 messages
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, Some("Be helpful".into()));
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
    }

    #[tokio::test]
    async fn test_store_conversation_count() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        store.add_message("user1", "user", "Hello", None).await.unwrap();
        store.add_message("user2", "user", "Hi", None).await.unwrap();
        store.add_message("user3", "user", "Hey", None).await.unwrap();

        let count = store.conversation_count().await;
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_store_health_check() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));
        assert!(store.health_check().await);
    }

    #[tokio::test]
    async fn test_store_ttl_expiration() {
        let store = ConversationStore::new(100, Duration::from_millis(50));

        store.add_message("user1", "user", "Hello", None).await.unwrap();
        assert!(store.get("user1").await.unwrap().is_some());

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be expired now
        assert!(store.get("user1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_store_ttl_refresh_on_activity() {
        let store = ConversationStore::new(100, Duration::from_millis(100));

        store.add_message("user1", "user", "Hello", None).await.unwrap();

        // Wait half the TTL
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Add another message - should refresh TTL
        store.add_message("user1", "user", "Still here", None).await.unwrap();

        // Wait another 60ms (would have expired if TTL wasn't refreshed)
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Should still exist because TTL was refreshed
        assert!(store.get("user1").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_store_message_trimming() {
        let store = ConversationStore::new(3, Duration::from_secs(3600));

        for i in 1..=5 {
            store.add_message("user1", "user", &format!("Message {}", i), None).await.unwrap();
        }

        let conv = store.get("user1").await.unwrap().unwrap();
        assert_eq!(conv.messages.len(), 3);
        assert_eq!(conv.messages[0].content, Some("Message 3".into()));
        assert_eq!(conv.messages[2].content, Some("Message 5".into()));
    }

    #[tokio::test]
    async fn test_store_add_assistant_with_tools() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        let tool_calls = vec![
            StoredToolCall {
                id: "call-1".into(),
                name: "calculate".into(),
                arguments: r#"{"expression": "2+2"}"#.into(),
            },
            StoredToolCall {
                id: "call-2".into(),
                name: "get_weather".into(),
                arguments: r#"{"location": "Tokyo"}"#.into(),
            },
        ];

        store
            .add_assistant_with_tools("user1", Some("Let me check that for you"), &tool_calls)
            .await
            .unwrap();

        let conv = store.get("user1").await.unwrap().unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, "assistant");
        assert_eq!(conv.messages[0].content, Some("Let me check that for you".into()));
        assert!(conv.messages[0].tool_calls.is_some());

        let stored_calls = conv.messages[0].tool_calls.as_ref().unwrap();
        assert_eq!(stored_calls.len(), 2);
        assert_eq!(stored_calls[0].id, "call-1");
        assert_eq!(stored_calls[0].name, "calculate");
        assert_eq!(stored_calls[1].id, "call-2");
        assert_eq!(stored_calls[1].name, "get_weather");
    }

    #[tokio::test]
    async fn test_store_add_assistant_with_tools_no_content() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        let tool_calls = vec![StoredToolCall {
            id: "call-1".into(),
            name: "calculate".into(),
            arguments: r#"{"expression": "2+2"}"#.into(),
        }];

        store
            .add_assistant_with_tools("user1", None, &tool_calls)
            .await
            .unwrap();

        let conv = store.get("user1").await.unwrap().unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, "assistant");
        assert_eq!(conv.messages[0].content, None);
        assert!(conv.messages[0].tool_calls.is_some());
    }

    #[tokio::test]
    async fn test_store_add_tool_result() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        store
            .add_tool_result("user1", "call-123", "The result is 42")
            .await
            .unwrap();

        let conv = store.get("user1").await.unwrap().unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, "tool");
        assert_eq!(conv.messages[0].content, Some("The result is 42".into()));
        assert_eq!(conv.messages[0].tool_call_id, Some("call-123".into()));
        assert!(conv.messages[0].tool_calls.is_none());
    }

    #[tokio::test]
    async fn test_store_tool_message_flow() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        // User message
        store.add_message("user1", "user", "What is 2+2?", None).await.unwrap();

        // Assistant with tool call
        let tool_calls = vec![StoredToolCall {
            id: "call-1".into(),
            name: "calculate".into(),
            arguments: r#"{"expression": "2+2"}"#.into(),
        }];
        store
            .add_assistant_with_tools("user1", None, &tool_calls)
            .await
            .unwrap();

        // Tool result
        store
            .add_tool_result("user1", "call-1", "2+2 = 4")
            .await
            .unwrap();

        // Final assistant response
        store
            .add_message("user1", "assistant", "The answer is 4", None)
            .await
            .unwrap();

        let conv = store.get("user1").await.unwrap().unwrap();
        assert_eq!(conv.messages.len(), 4);
        assert_eq!(conv.messages[0].role, "user");
        assert_eq!(conv.messages[1].role, "assistant");
        assert!(conv.messages[1].tool_calls.is_some());
        assert_eq!(conv.messages[2].role, "tool");
        assert_eq!(conv.messages[2].tool_call_id, Some("call-1".into()));
        assert_eq!(conv.messages[3].role, "assistant");
    }

    #[tokio::test]
    async fn test_store_to_openai_messages_with_tools() {
        let store = ConversationStore::new(100, Duration::from_secs(3600));

        // User message
        store.add_message("user1", "user", "Calculate 2+2", Some("Be helpful")).await.unwrap();

        // Assistant with tool call
        let tool_calls = vec![StoredToolCall {
            id: "call-1".into(),
            name: "calculate".into(),
            arguments: r#"{"expression": "2+2"}"#.into(),
        }];
        store
            .add_assistant_with_tools("user1", None, &tool_calls)
            .await
            .unwrap();

        // Tool result
        store
            .add_tool_result("user1", "call-1", "4")
            .await
            .unwrap();

        let messages = store.to_openai_messages("user1", Some("Be helpful")).await.unwrap();

        // system + user + assistant + tool
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
        assert!(messages[2].tool_calls.is_some());
        assert_eq!(messages[2].tool_calls.as_ref().unwrap().len(), 1);
        assert_eq!(messages[3].role, "tool");
        assert_eq!(messages[3].tool_call_id, Some("call-1".into()));
    }
}

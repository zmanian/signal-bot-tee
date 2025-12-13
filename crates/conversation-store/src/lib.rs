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
        assert_eq!(msg.content, "Hello, world!");
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
        assert_eq!(msg.content, "Test message");
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
        assert_eq!(conv.messages[0].content, "Hello");
        assert_eq!(conv.messages[1].role, "assistant");
        assert_eq!(conv.messages[1].content, "Hi!");
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
        assert_eq!(conv.messages[0].content, "Message 6");
        assert_eq!(conv.messages[4].content, "Message 10");
    }

    #[test]
    fn test_conversation_trim_to_exact_limit() {
        let mut conv = Conversation::new("user123", None);
        for i in 1..=5 {
            conv.add_message("user", &format!("Message {}", i));
        }

        conv.trim(5);

        assert_eq!(conv.messages.len(), 5);
        assert_eq!(conv.messages[0].content, "Message 1");
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
            content: "You are a helpful assistant".into(),
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
        assert_eq!(conv.messages[0].content, "Hello");
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
        assert_eq!(messages[0].content, "Be helpful");
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
        assert_eq!(conv.messages[0].content, "Message 3");
        assert_eq!(conv.messages[2].content, "Message 5");
    }
}

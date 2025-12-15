//! In-memory registry implementation.

use super::{PhoneNumberRecord, RegistrationStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// In-memory phone number registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    /// Phone number records indexed by normalized phone number
    records: HashMap<String, PhoneNumberRecord>,
}

impl Registry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    /// Get a record by phone number.
    pub fn get(&self, phone_number: &str) -> Option<&PhoneNumberRecord> {
        self.records.get(phone_number)
    }

    /// Get a mutable record by phone number.
    pub fn get_mut(&mut self, phone_number: &str) -> Option<&mut PhoneNumberRecord> {
        self.records.get_mut(phone_number)
    }

    /// Insert or update a record.
    pub fn insert(&mut self, phone_number: String, record: PhoneNumberRecord) {
        self.records.insert(phone_number, record);
    }

    /// Remove a record.
    pub fn remove(&mut self, phone_number: &str) -> Option<PhoneNumberRecord> {
        self.records.remove(phone_number)
    }

    /// Check if a phone number is registered (verified status).
    pub fn is_registered(&self, phone_number: &str) -> bool {
        self.records
            .get(phone_number)
            .map(|r| r.status == RegistrationStatus::Verified)
            .unwrap_or(false)
    }

    /// Check if a phone number has a pending registration.
    pub fn is_pending(&self, phone_number: &str) -> bool {
        self.records
            .get(phone_number)
            .map(|r| r.status == RegistrationStatus::Pending)
            .unwrap_or(false)
    }

    /// List all registered phone numbers.
    pub fn list_registered(&self) -> Vec<&PhoneNumberRecord> {
        self.records
            .values()
            .filter(|r| r.status == RegistrationStatus::Verified)
            .collect()
    }

    /// List all records (any status).
    pub fn list_all(&self) -> Vec<&PhoneNumberRecord> {
        self.records.values().collect()
    }

    /// Get the number of registered phone numbers.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// Get the number of verified registrations.
    pub fn count_verified(&self) -> usize {
        self.records
            .values()
            .filter(|r| r.status == RegistrationStatus::Verified)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_insert_and_get() {
        let mut registry = Registry::new();
        let record = PhoneNumberRecord::new_pending("+14155551234".into(), None);

        registry.insert("+14155551234".into(), record.clone());

        let retrieved = registry.get("+14155551234").unwrap();
        assert_eq!(retrieved.phone_number, "+14155551234");
        assert_eq!(retrieved.status, RegistrationStatus::Pending);
    }

    #[test]
    fn test_registry_is_registered() {
        let mut registry = Registry::new();
        let mut record = PhoneNumberRecord::new_pending("+14155551234".into(), None);

        registry.insert("+14155551234".into(), record.clone());
        assert!(!registry.is_registered("+14155551234"));

        record.mark_verified();
        registry.insert("+14155551234".into(), record);
        assert!(registry.is_registered("+14155551234"));
    }

    #[test]
    fn test_registry_is_pending() {
        let mut registry = Registry::new();
        let record = PhoneNumberRecord::new_pending("+14155551234".into(), None);

        registry.insert("+14155551234".into(), record);
        assert!(registry.is_pending("+14155551234"));
        assert!(!registry.is_pending("+19999999999"));
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = Registry::new();
        let record = PhoneNumberRecord::new_pending("+14155551234".into(), None);

        registry.insert("+14155551234".into(), record);
        assert!(registry.get("+14155551234").is_some());

        registry.remove("+14155551234");
        assert!(registry.get("+14155551234").is_none());
    }

    #[test]
    fn test_registry_serialization() {
        let mut registry = Registry::new();
        let record = PhoneNumberRecord::new_pending("+14155551234".into(), Some("secret"));
        registry.insert("+14155551234".into(), record);

        let json = serde_json::to_string(&registry).unwrap();
        let deserialized: Registry = serde_json::from_str(&json).unwrap();

        assert!(deserialized.get("+14155551234").is_some());
    }
}

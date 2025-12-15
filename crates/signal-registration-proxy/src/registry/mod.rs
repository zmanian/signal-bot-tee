//! Phone number registry with encrypted persistence.

mod encrypted;
mod memory;

pub use encrypted::{EncryptedStore, MemoryStore, Store};
pub use memory::Registry;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Registration status for a phone number.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RegistrationStatus {
    /// Registration initiated, awaiting verification code
    Pending,
    /// Verification code submitted, registration complete
    Verified,
    /// Registration failed or was abandoned
    Failed,
}

/// A registered phone number record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneNumberRecord {
    /// The phone number in E.164 format (e.g., "+14155551234")
    pub phone_number: String,

    /// When the number was first registered
    pub registered_at: DateTime<Utc>,

    /// Registration status
    pub status: RegistrationStatus,

    /// SHA-256 hash of ownership proof secret (if provided)
    pub ownership_proof_hash: Option<String>,
}

impl PhoneNumberRecord {
    /// Create a new pending registration record.
    pub fn new_pending(phone_number: String, ownership_secret: Option<&str>) -> Self {
        Self {
            phone_number,
            registered_at: Utc::now(),
            status: RegistrationStatus::Pending,
            ownership_proof_hash: ownership_secret.map(hash_secret),
        }
    }

    /// Check if the provided secret matches the stored ownership proof.
    pub fn verify_ownership(&self, secret: Option<&str>) -> bool {
        match (&self.ownership_proof_hash, secret) {
            (None, None) => true,
            (None, Some(_)) => true, // No proof required, any secret is fine
            (Some(_), None) => false, // Proof required but not provided
            (Some(stored), Some(provided)) => &hash_secret(provided) == stored,
        }
    }

    /// Mark registration as verified.
    pub fn mark_verified(&mut self) {
        self.status = RegistrationStatus::Verified;
    }

    /// Mark registration as failed.
    pub fn mark_failed(&mut self) {
        self.status = RegistrationStatus::Failed;
    }
}

/// Hash a secret using SHA-256.
pub fn hash_secret(secret: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

/// Normalize a phone number to E.164 format.
pub fn normalize_phone_number(number: &str) -> Result<String, String> {
    // Remove all non-digit characters except leading +
    let has_plus = number.starts_with('+');
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        return Err("Phone number must contain at least one digit".into());
    }

    if digits.len() < 7 {
        return Err("Phone number too short".into());
    }

    if digits.len() > 15 {
        return Err("Phone number too long".into());
    }

    // Ensure E.164 format starts with +
    if has_plus || digits.len() >= 10 {
        Ok(format!("+{}", digits))
    } else {
        Err("Phone number must include country code".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_secret() {
        let hash1 = hash_secret("test");
        let hash2 = hash_secret("test");
        let hash3 = hash_secret("different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA-256 produces 32 bytes = 64 hex chars
    }

    #[test]
    fn test_normalize_phone_number() {
        assert_eq!(
            normalize_phone_number("+1 (415) 555-1234"),
            Ok("+14155551234".into())
        );
        assert_eq!(
            normalize_phone_number("+14155551234"),
            Ok("+14155551234".into())
        );
        assert_eq!(
            normalize_phone_number("14155551234"),
            Ok("+14155551234".into())
        );
        assert!(normalize_phone_number("123").is_err());
        assert!(normalize_phone_number("").is_err());
    }

    #[test]
    fn test_verify_ownership() {
        let record = PhoneNumberRecord::new_pending("+14155551234".into(), Some("secret123"));

        assert!(record.verify_ownership(Some("secret123")));
        assert!(!record.verify_ownership(Some("wrong")));
        assert!(!record.verify_ownership(None));

        let no_proof_record = PhoneNumberRecord::new_pending("+14155551234".into(), None);
        assert!(no_proof_record.verify_ownership(None));
        assert!(no_proof_record.verify_ownership(Some("anything")));
    }

    #[test]
    fn test_registration_status_serialization() {
        let json = serde_json::to_string(&RegistrationStatus::Pending).unwrap();
        assert_eq!(json, "\"pending\"");

        let json = serde_json::to_string(&RegistrationStatus::Verified).unwrap();
        assert_eq!(json, "\"verified\"");
    }
}

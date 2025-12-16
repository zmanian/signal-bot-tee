//! TEE-encrypted persistent storage for the registry.

use super::Registry;
use crate::error::ProxyError;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use dstack_client::DstackClient;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info, warn};

/// Key derivation path for registry encryption.
const KEY_DERIVATION_PATH: &str = "signal-registration-proxy/registry";

/// Nonce size for AES-GCM (96 bits = 12 bytes).
const NONCE_SIZE: usize = 12;

/// TEE-encrypted persistent store for the registry.
pub struct EncryptedStore {
    dstack: DstackClient,
    storage_path: PathBuf,
    /// Cached key derived from AppInfo (used when DeriveKey not available)
    cached_key: Option<[u8; 32]>,
}

impl EncryptedStore {
    /// Create a new encrypted store.
    pub fn new(dstack: DstackClient, storage_path: PathBuf) -> Self {
        Self {
            dstack,
            storage_path,
            cached_key: None,
        }
    }

    /// Create a new encrypted store with a pre-derived key.
    pub fn with_key(dstack: DstackClient, storage_path: PathBuf, key: [u8; 32]) -> Self {
        Self {
            dstack,
            storage_path,
            cached_key: Some(key),
        }
    }

    /// Derive a 32-byte encryption key from TEE root of trust.
    ///
    /// The key is deterministic for the same TEE deployment (same compose hash),
    /// which means:
    /// - Same deployment can always decrypt its data
    /// - Different deployment (modified compose) cannot decrypt old data
    ///
    /// Tries DeriveKey endpoint first; falls back to deriving from AppInfo
    /// (compose_hash + app_id) if DeriveKey is not available.
    async fn derive_key(&self) -> Result<[u8; 32], ProxyError> {
        // Use cached key if available
        if let Some(key) = self.cached_key {
            debug!("Using cached encryption key");
            return Ok(key);
        }

        // Try the DeriveKey endpoint first
        match self.dstack.derive_key(KEY_DERIVATION_PATH, None).await {
            Ok(key_bytes) => {
                if key_bytes.len() < 32 {
                    return Err(ProxyError::Encryption(format!(
                        "Derived key too short: {} bytes (need 32)",
                        key_bytes.len()
                    )));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes[..32]);
                info!("Using DeriveKey endpoint for encryption key");
                return Ok(key);
            }
            Err(e) => {
                warn!(
                    "DeriveKey endpoint not available, falling back to AppInfo-derived key: {}",
                    e
                );
            }
        }

        // Fall back to deriving key from AppInfo (compose_hash + app_id)
        let app_info = self.dstack.get_app_info().await.map_err(|e| {
            ProxyError::Encryption(format!("Failed to get AppInfo for key derivation: {}", e))
        })?;

        let compose_hash = app_info.compose_hash.as_deref().unwrap_or("unknown");
        let app_id = app_info.app_id.as_deref().unwrap_or("unknown");

        // Derive key: SHA256(compose_hash || app_id || key_derivation_path)
        let mut hasher = Sha256::new();
        hasher.update(compose_hash.as_bytes());
        hasher.update(app_id.as_bytes());
        hasher.update(KEY_DERIVATION_PATH.as_bytes());
        let hash = hasher.finalize();

        let mut key = [0u8; 32];
        key.copy_from_slice(&hash);

        info!(
            "Using AppInfo-derived encryption key (compose_hash: {}, app_id: {})",
            compose_hash, app_id
        );

        Ok(key)
    }

    /// Save the registry to encrypted persistent storage.
    ///
    /// File format: [12 bytes nonce][ciphertext with auth tag]
    pub async fn save(&self, registry: &Registry) -> Result<(), ProxyError> {
        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Serialize the registry
        let plaintext = serde_json::to_vec(registry)?;

        // Encrypt with authenticated encryption
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())?;

        // Combine nonce + ciphertext
        let mut data = nonce_bytes.to_vec();
        data.extend(ciphertext);

        // Ensure parent directory exists
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Write atomically using temp file + rename
        let temp_path = self.storage_path.with_extension("tmp");
        fs::write(&temp_path, &data).await?;
        fs::rename(&temp_path, &self.storage_path).await?;

        debug!(
            "Saved encrypted registry ({} bytes) to {:?}",
            data.len(),
            self.storage_path
        );
        Ok(())
    }

    /// Load the registry from encrypted persistent storage.
    ///
    /// Returns an empty registry if the file doesn't exist.
    pub async fn load(&self) -> Result<Registry, ProxyError> {
        // Check if file exists
        if !self.storage_path.exists() {
            info!(
                "Registry file not found at {:?}, starting with empty registry",
                self.storage_path
            );
            return Ok(Registry::new());
        }

        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        let data = fs::read(&self.storage_path).await?;

        if data.len() < NONCE_SIZE {
            warn!("Registry file too short, starting with empty registry");
            return Ok(Registry::new());
        }

        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
        let ciphertext = &data[NONCE_SIZE..];

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
            ProxyError::Encryption(
                "Failed to decrypt registry. This may happen if the TEE deployment changed."
                    .to_string(),
            )
        })?;

        let registry: Registry = serde_json::from_slice(&plaintext)?;

        info!(
            "Loaded encrypted registry with {} records from {:?}",
            registry.count(),
            self.storage_path
        );
        Ok(registry)
    }

    /// Check if a registry file exists.
    pub fn exists(&self) -> bool {
        self.storage_path.exists()
    }
}

/// In-memory store for testing or when TEE is not available.
pub struct MemoryStore;

impl MemoryStore {
    /// "Save" does nothing for memory store.
    pub async fn save(&self, _registry: &Registry) -> Result<(), ProxyError> {
        debug!("Memory store: save is a no-op");
        Ok(())
    }

    /// "Load" returns an empty registry.
    pub async fn load(&self) -> Result<Registry, ProxyError> {
        debug!("Memory store: returning empty registry");
        Ok(Registry::new())
    }
}

/// Direct encryption/decryption with a known key (for testing).
#[cfg(test)]
pub mod testing {
    use super::*;

    /// Encrypt data with a known key (for testing).
    pub fn encrypt_with_key(data: &[u8], key: &[u8; 32]) -> Vec<u8> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data).expect("encryption failed");

        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        result
    }

    /// Decrypt data with a known key (for testing).
    pub fn decrypt_with_key(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, ProxyError> {
        if encrypted.len() < NONCE_SIZE {
            return Err(ProxyError::Encryption("Data too short".into()));
        }

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let nonce = Nonce::from_slice(&encrypted[..NONCE_SIZE]);
        let ciphertext = &encrypted[NONCE_SIZE..];

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| ProxyError::Encryption("Decryption failed".into()))
    }
}

/// Storage backend that works with or without TEE.
pub enum Store {
    /// TEE-encrypted file storage
    Encrypted(EncryptedStore),
    /// In-memory only (no persistence)
    Memory(MemoryStore),
}

impl Store {
    /// Create an encrypted store if TEE is available, otherwise memory store.
    pub async fn new(dstack: DstackClient, storage_path: PathBuf) -> Self {
        if dstack.is_in_tee().await {
            info!("Running in TEE, using encrypted persistent storage");
            Store::Encrypted(EncryptedStore::new(dstack, storage_path))
        } else {
            warn!("Not running in TEE, using in-memory storage (data will be lost on restart)");
            Store::Memory(MemoryStore)
        }
    }

    /// Force encrypted store (for testing with mock dstack).
    pub fn encrypted(dstack: DstackClient, storage_path: PathBuf) -> Self {
        Store::Encrypted(EncryptedStore::new(dstack, storage_path))
    }

    /// Force memory store.
    pub fn memory() -> Self {
        Store::Memory(MemoryStore)
    }

    /// Save the registry.
    pub async fn save(&self, registry: &Registry) -> Result<(), ProxyError> {
        match self {
            Store::Encrypted(s) => s.save(registry).await,
            Store::Memory(s) => s.save(registry).await,
        }
    }

    /// Load the registry.
    pub async fn load(&self) -> Result<Registry, ProxyError> {
        match self {
            Store::Encrypted(s) => s.load().await,
            Store::Memory(s) => s.load().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::PhoneNumberRecord;

    #[test]
    fn test_encryption_round_trip() {
        let key = [0x42u8; 32]; // Test key
        let data = b"Hello, World!";

        let encrypted = testing::encrypt_with_key(data, &key);
        assert_ne!(encrypted, data); // Should be different
        assert!(encrypted.len() > data.len()); // Should be longer (nonce + tag)

        let decrypted = testing::decrypt_with_key(&encrypted, &key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encryption_different_keys() {
        let key1 = [0x42u8; 32];
        let key2 = [0x43u8; 32];
        let data = b"Secret data";

        let encrypted = testing::encrypt_with_key(data, &key1);

        // Should fail with wrong key
        let result = testing::decrypt_with_key(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn test_encryption_tamper_detection() {
        let key = [0x42u8; 32];
        let data = b"Sensitive information";

        let mut encrypted = testing::encrypt_with_key(data, &key);

        // Tamper with the ciphertext
        if let Some(byte) = encrypted.last_mut() {
            *byte ^= 0xFF;
        }

        // Should fail because of authentication tag
        let result = testing::decrypt_with_key(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_serialization_round_trip() {
        let mut registry = Registry::new();
        let record = PhoneNumberRecord::new_pending("+14155551234".into(), Some("secret"));
        registry.insert("+14155551234".into(), record);

        // Serialize
        let json = serde_json::to_vec(&registry).unwrap();

        // Encrypt
        let key = [0x42u8; 32];
        let encrypted = testing::encrypt_with_key(&json, &key);

        // Decrypt
        let decrypted = testing::decrypt_with_key(&encrypted, &key).unwrap();

        // Deserialize
        let restored: Registry = serde_json::from_slice(&decrypted).unwrap();

        assert!(restored.get("+14155551234").is_some());
        assert!(restored.get("+14155551234").unwrap().verify_ownership(Some("secret")));
    }

    #[test]
    fn test_memory_store_operations() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = MemoryStore;

            // Load returns empty registry
            let registry = store.load().await.unwrap();
            assert_eq!(registry.count(), 0);

            // Save is a no-op
            let mut registry = Registry::new();
            registry.insert(
                "+14155551234".into(),
                PhoneNumberRecord::new_pending("+14155551234".into(), None),
            );
            store.save(&registry).await.unwrap();

            // Load still returns empty (no persistence)
            let registry = store.load().await.unwrap();
            assert_eq!(registry.count(), 0);
        });
    }

    #[test]
    fn test_store_memory_variant() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = Store::memory();

            let registry = store.load().await.unwrap();
            assert_eq!(registry.count(), 0);

            let mut registry = Registry::new();
            registry.insert(
                "+14155551234".into(),
                PhoneNumberRecord::new_pending("+14155551234".into(), None),
            );

            // Save succeeds
            store.save(&registry).await.unwrap();
        });
    }
}

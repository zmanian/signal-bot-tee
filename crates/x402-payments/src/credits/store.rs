//! TEE-encrypted persistent credit store.

use crate::error::PaymentError;
use crate::types::{CreditBalance, Deposit, UsageRecord, UserId};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use dstack_client::DstackClient;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Key derivation path for credit store encryption.
const KEY_DERIVATION_PATH: &str = "x402-payments/credit-store";

/// Nonce size for AES-GCM (96 bits = 12 bytes).
const NONCE_SIZE: usize = 12;

/// Data version for schema migrations.
const DATA_VERSION: u32 = 1;

/// Persistent data structure for the credit store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditStoreData {
    /// Schema version for migrations.
    pub version: u32,
    /// User credit balances.
    pub balances: HashMap<UserId, CreditBalance>,
    /// Deposit history.
    pub deposits: Vec<Deposit>,
    /// Usage log (for auditing).
    pub usage_log: Vec<UsageRecord>,
    /// Processed transaction hashes (for double-spend prevention).
    pub processed_tx_hashes: HashSet<String>,
}

impl Default for CreditStoreData {
    fn default() -> Self {
        Self {
            version: DATA_VERSION,
            balances: HashMap::new(),
            deposits: Vec::new(),
            usage_log: Vec::new(),
            processed_tx_hashes: HashSet::new(),
        }
    }
}

/// TEE-encrypted credit store.
pub struct CreditStore {
    data: RwLock<CreditStoreData>,
    dstack: DstackClient,
    storage_path: PathBuf,
    /// Cached encryption key.
    cached_key: RwLock<Option<[u8; 32]>>,
}

impl CreditStore {
    /// Create a new credit store and load existing data if available.
    pub async fn new(dstack: DstackClient, storage_path: PathBuf) -> Result<Arc<Self>, PaymentError> {
        let store = Arc::new(Self {
            data: RwLock::new(CreditStoreData::default()),
            dstack,
            storage_path,
            cached_key: RwLock::new(None),
        });

        // Load existing data if available
        store.load().await?;

        Ok(store)
    }

    /// Create a credit store with a pre-derived key (for testing).
    pub async fn with_key(
        dstack: DstackClient,
        storage_path: PathBuf,
        key: [u8; 32],
    ) -> Result<Arc<Self>, PaymentError> {
        let store = Arc::new(Self {
            data: RwLock::new(CreditStoreData::default()),
            dstack,
            storage_path,
            cached_key: RwLock::new(Some(key)),
        });

        store.load().await?;

        Ok(store)
    }

    /// Derive encryption key from TEE root of trust.
    async fn derive_key(&self) -> Result<[u8; 32], PaymentError> {
        // Check cache first
        {
            let cached = self.cached_key.read().await;
            if let Some(key) = *cached {
                return Ok(key);
            }
        }

        // Try DeriveKey endpoint first
        match self.dstack.derive_key(KEY_DERIVATION_PATH, None).await {
            Ok(key_bytes) => {
                if key_bytes.len() < 32 {
                    return Err(PaymentError::Encryption(format!(
                        "Derived key too short: {} bytes",
                        key_bytes.len()
                    )));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes[..32]);

                // Cache the key
                *self.cached_key.write().await = Some(key);

                info!("Using DeriveKey endpoint for credit store encryption");
                return Ok(key);
            }
            Err(e) => {
                warn!(
                    "DeriveKey not available, falling back to AppInfo: {}",
                    e
                );
            }
        }

        // Fallback to AppInfo-derived key
        let app_info = self.dstack.get_app_info().await.map_err(|e| {
            PaymentError::Encryption(format!("Failed to get AppInfo: {}", e))
        })?;

        let compose_hash = app_info.compose_hash.as_deref().unwrap_or("unknown");
        let app_id = app_info.app_id.as_deref().unwrap_or("unknown");

        let mut hasher = Sha256::new();
        hasher.update(compose_hash.as_bytes());
        hasher.update(app_id.as_bytes());
        hasher.update(KEY_DERIVATION_PATH.as_bytes());
        let hash = hasher.finalize();

        let mut key = [0u8; 32];
        key.copy_from_slice(&hash);

        // Cache the key
        *self.cached_key.write().await = Some(key);

        info!(
            "Using AppInfo-derived key (compose_hash: {}, app_id: {})",
            compose_hash, app_id
        );

        Ok(key)
    }

    /// Save data to encrypted storage.
    pub async fn persist(&self) -> Result<(), PaymentError> {
        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Serialize data
        let data = self.data.read().await;
        let plaintext = serde_json::to_vec(&*data)?;

        // Encrypt
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())?;

        // Combine nonce + ciphertext
        let mut encrypted = nonce_bytes.to_vec();
        encrypted.extend(ciphertext);

        // Ensure parent directory exists
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Atomic write
        let temp_path = self.storage_path.with_extension("tmp");
        fs::write(&temp_path, &encrypted).await?;
        fs::rename(&temp_path, &self.storage_path).await?;

        debug!(
            "Saved credit store ({} bytes) to {:?}",
            encrypted.len(),
            self.storage_path
        );

        Ok(())
    }

    /// Load data from encrypted storage.
    async fn load(&self) -> Result<(), PaymentError> {
        if !self.storage_path.exists() {
            info!(
                "Credit store not found at {:?}, starting fresh",
                self.storage_path
            );
            return Ok(());
        }

        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        let encrypted = fs::read(&self.storage_path).await?;

        if encrypted.len() < NONCE_SIZE {
            warn!("Credit store file too short, starting fresh");
            return Ok(());
        }

        let nonce = Nonce::from_slice(&encrypted[..NONCE_SIZE]);
        let ciphertext = &encrypted[NONCE_SIZE..];

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
            PaymentError::Encryption(
                "Failed to decrypt credit store. TEE deployment may have changed.".to_string(),
            )
        })?;

        let data: CreditStoreData = serde_json::from_slice(&plaintext)?;

        info!(
            "Loaded credit store: {} balances, {} deposits",
            data.balances.len(),
            data.deposits.len()
        );

        *self.data.write().await = data;

        Ok(())
    }

    /// Get credit balance for a user.
    pub async fn get_balance(&self, user_id: &str) -> CreditBalance {
        let data = self.data.read().await;
        data.balances
            .get(user_id)
            .cloned()
            .unwrap_or_else(|| CreditBalance::new(user_id.to_string()))
    }

    /// Check if user has sufficient credits.
    pub async fn has_credits(&self, user_id: &str, required: u64) -> bool {
        let balance = self.get_balance(user_id).await;
        balance.has_credits(required)
    }

    /// Add credits from a deposit.
    pub async fn add_credits(
        &self,
        deposit: Deposit,
    ) -> Result<CreditBalance, PaymentError> {
        let balance_clone = {
            let mut data = self.data.write().await;

            // Check for double-spend
            if data.processed_tx_hashes.contains(&deposit.tx_hash) {
                return Err(PaymentError::DuplicateTransaction(deposit.tx_hash.clone()));
            }

            // Record deposit first to avoid borrow issues
            data.processed_tx_hashes.insert(deposit.tx_hash.clone());
            let credits_granted = deposit.credits_granted;
            let user_id = deposit.user_id.clone();
            data.deposits.push(deposit);

            // Get or create balance and add credits
            let balance = data
                .balances
                .entry(user_id.clone())
                .or_insert_with(|| CreditBalance::new(user_id));

            balance.add_credits(credits_granted);
            balance.clone()
        };

        // Persist (lock is released)
        self.persist().await?;

        Ok(balance_clone)
    }

    /// Deduct credits for usage.
    pub async fn deduct_credits(
        &self,
        user_id: &str,
        credits: u64,
        usage: UsageRecord,
    ) -> Result<CreditBalance, PaymentError> {
        let balance_clone = {
            let mut data = self.data.write().await;

            // First check if user exists and has enough credits
            let available = data
                .balances
                .get(user_id)
                .map(|b| b.credits_remaining)
                .unwrap_or(0);

            if available < credits {
                return Err(PaymentError::InsufficientCredits {
                    required: credits,
                    available,
                });
            }

            // Now get mutable reference, record usage first
            data.usage_log.push(usage);

            // Then deduct credits
            let balance = data
                .balances
                .get_mut(user_id)
                .ok_or_else(|| PaymentError::UserNotFound(user_id.to_string()))?;

            balance.deduct_credits(credits);
            balance.clone()
        };

        // Persist (lock is released)
        self.persist().await?;

        Ok(balance_clone)
    }

    /// Get deposits for a user.
    pub async fn get_deposits(&self, user_id: &str) -> Vec<Deposit> {
        let data = self.data.read().await;
        data.deposits
            .iter()
            .filter(|d| d.user_id == user_id)
            .cloned()
            .collect()
    }

    /// Get usage records for a user.
    pub async fn get_usage(&self, user_id: &str) -> Vec<UsageRecord> {
        let data = self.data.read().await;
        data.usage_log
            .iter()
            .filter(|u| u.user_id == user_id)
            .cloned()
            .collect()
    }

    /// Check if a transaction has been processed.
    pub async fn is_tx_processed(&self, tx_hash: &str) -> bool {
        let data = self.data.read().await;
        data.processed_tx_hashes.contains(tx_hash)
    }

    /// Get summary statistics.
    pub async fn get_stats(&self) -> CreditStoreStats {
        let data = self.data.read().await;
        CreditStoreStats {
            total_users: data.balances.len(),
            total_deposits: data.deposits.len(),
            total_usage_records: data.usage_log.len(),
            total_credits_deposited: data.deposits.iter().map(|d| d.credits_granted).sum(),
            total_credits_consumed: data.usage_log.iter().map(|u| u.credits_consumed).sum(),
        }
    }
}

/// Summary statistics for the credit store.
#[derive(Debug, Clone)]
pub struct CreditStoreStats {
    pub total_users: usize,
    pub total_deposits: usize,
    pub total_usage_records: usize,
    pub total_credits_deposited: u64,
    pub total_credits_consumed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Chain;
    use tempfile::TempDir;

    fn create_test_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    async fn create_test_store() -> (Arc<CreditStore>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("credits.enc");
        // Use a non-existent socket path for testing (won't be used with pre-derived key)
        let dstack = DstackClient::new("/var/run/dstack.sock");

        let store = CreditStore::with_key(dstack, storage_path, create_test_key())
            .await
            .unwrap();

        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_get_balance_new_user() {
        let (store, _dir) = create_test_store().await;

        let balance = store.get_balance("+14155551234").await;

        assert_eq!(balance.user_id, "+14155551234");
        assert_eq!(balance.credits_remaining, 0);
    }

    #[tokio::test]
    async fn test_add_credits() {
        let (store, _dir) = create_test_store().await;

        let deposit = Deposit::new_pending(
            "+14155551234".to_string(),
            Chain::Base,
            "0x123abc".to_string(),
            1_000_000, // 1 USDC
            1_000_000, // 1M credits
        );

        let balance = store.add_credits(deposit).await.unwrap();

        assert_eq!(balance.credits_remaining, 1_000_000);
        assert_eq!(balance.total_deposited, 1_000_000);
    }

    #[tokio::test]
    async fn test_deduct_credits() {
        let (store, _dir) = create_test_store().await;

        // First add credits
        let deposit = Deposit::new_pending(
            "+14155551234".to_string(),
            Chain::Base,
            "0x123abc".to_string(),
            1_000_000,
            1_000_000,
        );
        store.add_credits(deposit).await.unwrap();

        // Now deduct
        let usage = UsageRecord::new(
            "+14155551234".to_string(),
            "+14155551234".to_string(),
            1000,
            500,
            500, // 500 credits
        );

        let balance = store.deduct_credits("+14155551234", 500, usage).await.unwrap();

        assert_eq!(balance.credits_remaining, 999_500);
        assert_eq!(balance.total_consumed, 500);
    }

    #[tokio::test]
    async fn test_insufficient_credits() {
        let (store, _dir) = create_test_store().await;

        // Add some credits
        let deposit = Deposit::new_pending(
            "+14155551234".to_string(),
            Chain::Base,
            "0x123abc".to_string(),
            100,
            100,
        );
        store.add_credits(deposit).await.unwrap();

        // Try to deduct more than available
        let usage = UsageRecord::new(
            "+14155551234".to_string(),
            "+14155551234".to_string(),
            1000,
            500,
            200,
        );

        let result = store.deduct_credits("+14155551234", 200, usage).await;

        assert!(matches!(
            result,
            Err(PaymentError::InsufficientCredits { .. })
        ));
    }

    #[tokio::test]
    async fn test_double_spend_prevention() {
        let (store, _dir) = create_test_store().await;

        let deposit1 = Deposit::new_pending(
            "+14155551234".to_string(),
            Chain::Base,
            "0x123abc".to_string(),
            1_000_000,
            1_000_000,
        );

        // First deposit succeeds
        store.add_credits(deposit1).await.unwrap();

        // Same tx hash should fail
        let deposit2 = Deposit::new_pending(
            "+14155551234".to_string(),
            Chain::Base,
            "0x123abc".to_string(), // Same tx hash!
            1_000_000,
            1_000_000,
        );

        let result = store.add_credits(deposit2).await;

        assert!(matches!(
            result,
            Err(PaymentError::DuplicateTransaction(_))
        ));
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("credits.enc");
        let key = create_test_key();

        // Create store and add data
        {
            let dstack = DstackClient::new("/var/run/dstack.sock");
            let store = CreditStore::with_key(dstack, storage_path.clone(), key)
                .await
                .unwrap();

            let deposit = Deposit::new_pending(
                "+14155551234".to_string(),
                Chain::Base,
                "0x123abc".to_string(),
                1_000_000,
                1_000_000,
            );
            store.add_credits(deposit).await.unwrap();
        }

        // Create new store instance and verify data loaded
        {
            let dstack = DstackClient::new("/var/run/dstack.sock");
            let store = CreditStore::with_key(dstack, storage_path, key)
                .await
                .unwrap();

            let balance = store.get_balance("+14155551234").await;
            assert_eq!(balance.credits_remaining, 1_000_000);
        }
    }

    #[tokio::test]
    async fn test_has_credits() {
        let (store, _dir) = create_test_store().await;

        // No credits initially
        assert!(!store.has_credits("+14155551234", 100).await);

        // Add credits
        let deposit = Deposit::new_pending(
            "+14155551234".to_string(),
            Chain::Base,
            "0x123abc".to_string(),
            1_000_000,
            1_000_000,
        );
        store.add_credits(deposit).await.unwrap();

        // Now has credits
        assert!(store.has_credits("+14155551234", 100).await);
        assert!(store.has_credits("+14155551234", 1_000_000).await);
        assert!(!store.has_credits("+14155551234", 1_000_001).await);
    }
}

# Signal API Registration Proxy - Implementation Plan

## Executive Summary

This plan describes a new HTTP proxy service that sits in front of the Signal CLI REST API's registration endpoints. The proxy allows anyone to register **new** phone numbers but prevents re-registration of numbers that have already been registered through this TEE instance. This is critical for a multi-tenant TEE Signal service where users can provision their own bot phone numbers.

## Problem Statement

The current architecture supports a single pre-configured phone number (`SIGNAL__PHONE_NUMBER`). For a multi-tenant TEE service, we need:

1. **Self-service registration**: Users can register their own phone numbers
2. **No re-registration attacks**: Once a number is registered, only the original registrant should be able to use it (prevents hijacking)
3. **TEE security**: Registration state must be protected within the TEE
4. **Persistence**: Registration state must survive container restarts

## Architecture Decision

### Option 1: New Crate (Recommended)

Create a new `signal-registration-proxy` crate that:
- Exposes an HTTP API for registration operations
- Maintains a registry of claimed phone numbers
- Proxies only safe operations to the underlying Signal CLI REST API
- Uses TEE-derived keys for encrypted persistent storage

**Pros:**
- Clean separation of concerns
- Can be deployed independently or with the bot
- Follows existing crate structure pattern

**Cons:**
- New binary to maintain

### Option 2: Extend `signal-client` crate

Add registration methods to the existing `signal-client` crate and handle access control in `signal-bot`.

**Pros:**
- Less code duplication
- Reuses existing HTTP client

**Cons:**
- Mixes bot concerns with proxy concerns
- The bot is designed for a single phone number
- Would require significant refactoring

### Option 3: Standalone HTTP proxy binary

A new binary crate `signal-registration-proxy` that acts as a reverse proxy.

**Pros:**
- Complete isolation
- Can be deployed in front of any Signal CLI REST API instance

**Cons:**
- Another container in the stack

**Decision: Option 1 (New Crate) with Option 3 deployment model**

Create `crates/signal-registration-proxy/` as a new binary crate that:
1. Exposes HTTP endpoints for registration
2. Maintains encrypted persistent storage of claimed numbers
3. Proxies to the Signal CLI REST API for actual registration
4. Runs as a separate container in the docker-compose

## Detailed Design

### 1. Phone Number Registry

#### Data Model

```rust
// crates/signal-registration-proxy/src/registry.rs

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// A registered phone number record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneNumberRecord {
    /// The phone number in E.164 format (e.g., "+14155551234")
    pub phone_number: String,

    /// When the number was first registered
    pub registered_at: SystemTime,

    /// Registration status
    pub status: RegistrationStatus,

    /// Optional: SHA-256 hash of a user-provided secret for ownership proof
    /// (allows the original registrant to prove ownership without storing secrets)
    pub ownership_proof_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegistrationStatus {
    /// Registration initiated, awaiting verification code
    Pending,
    /// Verification code submitted, registration complete
    Verified,
    /// Registration failed or was abandoned
    Failed,
}
```

#### Storage Strategy

**Problem**: The project explicitly avoids external persistence (Redis) for security reasons. However, the registration registry must survive container restarts.

**Solution**: TEE-encrypted file storage using Dstack key derivation:

1. **Derive encryption key** from TEE root of trust using `/DeriveKey` API
2. **Encrypt registry** with AES-256-GCM using derived key
3. **Store to Docker volume** (`signal-config` or new `registry-data` volume)
4. **On startup**, decrypt and load registry

```rust
// crates/signal-registration-proxy/src/encrypted_store.rs

use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use dstack_client::DstackClient;

pub struct EncryptedStore {
    dstack: DstackClient,
    storage_path: PathBuf,
}

impl EncryptedStore {
    /// Derive encryption key from TEE root of trust
    async fn derive_key(&self) -> Result<[u8; 32], StoreError> {
        // Path uniquely identifies this data type
        // Key is deterministic across restarts for same TEE deployment
        let key_bytes = self.dstack
            .derive_key("signal-registration-proxy/registry", None)
            .await?;

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes[..32]);
        Ok(key)
    }

    pub async fn save(&self, registry: &Registry) -> Result<(), StoreError> {
        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        // Generate random nonce
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Serialize and encrypt
        let plaintext = serde_json::to_vec(registry)?;
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())?;

        // Write: [12 bytes nonce][ciphertext]
        let mut data = nonce_bytes.to_vec();
        data.extend(ciphertext);

        tokio::fs::write(&self.storage_path, data).await?;
        Ok(())
    }

    pub async fn load(&self) -> Result<Registry, StoreError> {
        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        let data = tokio::fs::read(&self.storage_path).await?;

        if data.len() < 12 {
            return Err(StoreError::CorruptedData);
        }

        let nonce = Nonce::from_slice(&data[..12]);
        let ciphertext = &data[12..];

        let plaintext = cipher.decrypt(nonce, ciphertext)?;
        let registry: Registry = serde_json::from_slice(&plaintext)?;

        Ok(registry)
    }
}
```

**Key Properties:**
- **Deterministic key derivation**: Same TEE deployment always derives same key
- **Different deployments**: Different compose hash = different derived keys = cannot decrypt
- **At-rest encryption**: Even if volume is accessed outside TEE, data is encrypted
- **Integrity**: AES-GCM provides authenticated encryption

### 2. HTTP API Design

#### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/register/{number}` | Initiate registration for a new number |
| `POST` | `/v1/register/{number}/verify/{code}` | Verify registration with SMS/voice code |
| `GET` | `/v1/status/{number}` | Check registration status |
| `GET` | `/v1/accounts` | List all registered accounts |
| `DELETE` | `/v1/unregister/{number}` | Unregister a number (requires ownership proof) |
| `GET` | `/v1/qrcodelink` | Generate QR code for device linking (blocked for registered numbers) |
| `GET` | `/health` | Health check |

#### Request/Response Types

```rust
// crates/signal-registration-proxy/src/api/types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Optional CAPTCHA token if required by Signal
    pub captcha: Option<String>,

    /// Use voice call instead of SMS for verification code
    #[serde(default)]
    pub use_voice: bool,

    /// Optional ownership proof secret (will be hashed and stored)
    /// Required for later unregistration or re-registration
    pub ownership_secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub phone_number: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    /// Optional Signal PIN to set
    pub pin: Option<String>,

    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub phone_number: String,
    pub status: RegistrationStatus,
    pub registered_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}
```

#### Access Control Logic

```rust
// crates/signal-registration-proxy/src/api/handlers.rs

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

pub async fn register_number(
    State(state): State<AppState>,
    Path(number): Path<String>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    // Normalize phone number
    let number = normalize_phone_number(&number)?;

    // Check if already registered
    let registry = state.registry.read().await;
    if let Some(record) = registry.get(&number) {
        match record.status {
            RegistrationStatus::Verified => {
                return Err(ApiError::AlreadyRegistered {
                    number: number.clone(),
                    message: "This phone number is already registered. Use the existing account or contact support.".into(),
                });
            }
            RegistrationStatus::Pending => {
                // Allow retry if pending (verification timed out)
                // But only if ownership_secret matches (if one was set)
                if let Some(ref stored_hash) = record.ownership_proof_hash {
                    let provided_hash = request.ownership_secret
                        .as_ref()
                        .map(|s| sha256_hash(s));

                    if provided_hash.as_ref() != Some(stored_hash) {
                        return Err(ApiError::OwnershipProofMismatch);
                    }
                }
            }
            RegistrationStatus::Failed => {
                // Allow re-registration for failed attempts
            }
        }
    }
    drop(registry);

    // Proxy to Signal CLI REST API
    let signal_response = state.signal_client
        .register(&number, request.captcha.as_deref(), request.use_voice)
        .await?;

    // Record the registration attempt
    let mut registry = state.registry.write().await;
    registry.insert(number.clone(), PhoneNumberRecord {
        phone_number: number.clone(),
        registered_at: SystemTime::now(),
        status: RegistrationStatus::Pending,
        ownership_proof_hash: request.ownership_secret.map(|s| sha256_hash(&s)),
    });

    // Persist to encrypted storage
    state.store.save(&registry).await?;

    Ok(Json(RegisterResponse {
        phone_number: number,
        status: "pending".into(),
        message: "Verification code sent. Use /v1/register/{number}/verify/{code} to complete.".into(),
    }))
}
```

### 3. Security Considerations

#### Rate Limiting

```rust
// crates/signal-registration-proxy/src/middleware/rate_limit.rs

use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

/// Rate limits by client IP
pub struct RateLimitLayer {
    /// Global rate limit: max 10 registration attempts per minute
    global_limiter: RateLimiter<String>,

    /// Per-number rate limit: max 3 attempts per hour per phone number
    per_number_limiter: RateLimiter<String>,
}

impl RateLimitLayer {
    pub fn new() -> Self {
        Self {
            global_limiter: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(10).unwrap())
            ),
            per_number_limiter: RateLimiter::keyed(
                Quota::per_hour(NonZeroU32::new(3).unwrap())
            ),
        }
    }
}
```

#### Abuse Prevention

1. **Ownership proof**: Optional secret hashed and stored; required for sensitive operations
2. **Phone number normalization**: Prevent bypass via formatting variations
3. **Signal's built-in CAPTCHA**: Proxy supports CAPTCHA flow
4. **Audit logging**: All registration attempts logged (inside TEE)

#### TEE Implications

1. **Key derivation path**: Using `"signal-registration-proxy/registry"` ensures:
   - Different apps in same TEE get different keys
   - Same app with different compose hash gets different keys

2. **No external dependencies**: Unlike Redis, the encrypted file approach keeps all secrets in TEE memory during operation

3. **Attestation**: Users can verify the proxy code via `!verify` command (if compose includes both containers)

### 4. Project Structure

```
crates/signal-registration-proxy/
  Cargo.toml
  src/
    main.rs              # Entry point, server setup
    config.rs            # Configuration from environment
    error.rs             # Error types
    lib.rs               # Library exports

    registry/
      mod.rs             # Registry trait and types
      memory.rs          # In-memory implementation
      encrypted.rs       # TEE-encrypted file storage

    api/
      mod.rs             # Router setup
      types.rs           # Request/response types
      handlers.rs        # HTTP handlers
      middleware.rs      # Rate limiting, logging

    signal/
      mod.rs             # Signal CLI client wrapper
      client.rs          # HTTP client to Signal API
      types.rs           # Signal API types
```

#### Cargo.toml

```toml
[package]
name = "signal-registration-proxy"
version.workspace = true
edition.workspace = true

[[bin]]
name = "signal-registration-proxy"
path = "src/main.rs"

[dependencies]
# Workspace crates
dstack-client = { path = "../dstack-client" }

# Workspace dependencies
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
thiserror.workspace = true
anyhow.workspace = true
config.workspace = true
dotenvy.workspace = true
reqwest.workspace = true
sha2.workspace = true
hex.workspace = true

# New dependencies
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
governor = "0.6"
aes-gcm = "0.10"
rand = "0.8"
```

### 5. Docker Compose Integration

```yaml
# Updated docker/docker-compose.yaml

version: "3.8"

services:
  signal-api:
    # Pinned to specific digest for TEE attestation integrity
    image: bbernhard/signal-cli-rest-api@sha256:04ee57f9819a90c89fbee46f74e080a032f5f05decf6e4b4a4e1f45d050ed9c8
    container_name: signal-api
    environment:
      - MODE=normal
      - AUTO_RECEIVE_SCHEDULE=
      - LOG_LEVEL=info
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock:ro
      - signal-config:/home/.local/share/signal-cli
    # SECURITY: No ports exposed - only accessible within Docker network
    networks:
      - internal
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
    restart: unless-stopped

  signal-registration-proxy:
    build:
      context: ..
      dockerfile: docker/Dockerfile.proxy
    container_name: signal-registration-proxy
    environment:
      - SIGNAL_API_URL=http://signal-api:8080
      - REGISTRY_PATH=/data/registry.enc
      - DSTACK__SOCKET_PATH=/var/run/dstack.sock
      - LOG_LEVEL=info
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock:ro
      - registry-data:/data
    ports:
      # Expose registration API externally
      - "8081:8081"
    networks:
      - internal
    depends_on:
      signal-api:
        condition: service_healthy
    restart: unless-stopped

  signal-bot:
    build:
      context: ..
      dockerfile: docker/Dockerfile
    container_name: signal-bot
    environment:
      - SIGNAL__SERVICE_URL=http://signal-api:8080
      - SIGNAL__PHONE_NUMBER=${SIGNAL_PHONE}
      - NEAR_AI__API_KEY=${NEAR_AI_API_KEY}
      - NEAR_AI__BASE_URL=${NEAR_AI_BASE_URL:-https://cloud-api.near.ai/v1}
      - NEAR_AI__MODEL=${NEAR_AI_MODEL:-deepseek-ai/DeepSeek-V3.1}
      - CONVERSATION__MAX_MESSAGES=${CONVERSATION_MAX_MESSAGES:-50}
      - CONVERSATION__TTL=${CONVERSATION_TTL:-24h}
      - BOT__LOG_LEVEL=${LOG_LEVEL:-info}
      - DSTACK__SOCKET_PATH=/var/run/dstack.sock
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock:ro
    networks:
      - internal
    depends_on:
      signal-api:
        condition: service_healthy
    restart: unless-stopped

volumes:
  signal-config:
    driver: local
  registry-data:
    driver: local

networks:
  internal:
    name: signal-bot-internal
```

### 6. Persistence Across Restarts

**Scenario Analysis:**

| Event | Registry State | Encryption Key |
|-------|---------------|----------------|
| Container restart (same TEE) | Persisted on volume | Same (deterministic derivation) |
| TEE instance restart (same compose) | Persisted on volume | Same (same compose hash) |
| Deploy with modified compose | Persisted on volume | Different (different compose hash) |
| Volume deleted | Lost | N/A |

**Key insight**: The Dstack key derivation is deterministic based on:
1. The `path` parameter (`"signal-registration-proxy/registry"`)
2. The TEE measurement (compose hash)

This means:
- Same deployment can always decrypt its data
- Different deployment (modified compose) cannot decrypt old data
- This is actually a security feature: prevents migrating registration data to a tampered deployment

### 7. Implementation Sequence

#### Phase 1: Core Infrastructure
1. Create `crates/signal-registration-proxy/` directory structure
2. Implement `EncryptedStore` with TEE key derivation
3. Implement basic `Registry` with in-memory + encrypted persistence
4. Add unit tests for encryption/decryption round-trip

#### Phase 2: HTTP API
5. Set up Axum HTTP server
6. Implement `/health` endpoint
7. Implement `/v1/register/{number}` with access control
8. Implement `/v1/register/{number}/verify/{code}`
9. Implement `/v1/status/{number}`
10. Add rate limiting middleware

#### Phase 3: Signal CLI Integration
11. Create Signal CLI client wrapper (reuse from `signal-client` crate)
12. Add registration-specific methods
13. Handle CAPTCHA flow
14. Handle error responses

#### Phase 4: Docker Integration
15. Create `Dockerfile.proxy`
16. Update `docker-compose.yaml`
17. Update `phala-compose.yaml` for TEE deployment
18. Add integration tests

#### Phase 5: Documentation & Polish
19. Update README with registration proxy usage
20. Update CLAUDE.md with security analysis
21. Add OpenAPI/Swagger documentation
22. Add metrics/monitoring endpoints

### 8. Alternative Approaches Considered

#### In-Memory Only (No Persistence)

**Pros:** Simpler, follows existing pattern for conversation store
**Cons:** Registration state lost on restart; users would need to re-register

**Why rejected:** Registration is different from conversations. Users expect registered numbers to remain registered. Losing registration state would be a poor user experience and could cause confusion about which numbers are "claimed."

#### External KMS for Key Storage

**Pros:** More robust key management
**Cons:** Adds external dependency; complicates deployment; potential security boundary issue

**Why rejected:** Dstack's built-in key derivation is sufficient and keeps everything within the TEE trust boundary.

#### Blockchain-Based Registry

**Pros:** Immutable, publicly verifiable
**Cons:** Latency, cost, complexity; overkill for this use case

**Why rejected:** The threat model doesn't require public verifiability of registration. TEE attestation already proves the proxy is running correctly.

---

## Critical Files for Implementation

- `crates/dstack-client/src/client.rs` - Contains `derive_key()` method needed for TEE-encrypted storage
- `crates/signal-client/src/client.rs` - Pattern for HTTP client to Signal CLI REST API; can be extended or used as reference
- `crates/signal-bot/src/config.rs` - Configuration pattern to follow for the new crate
- `docker/docker-compose.yaml` - Must be updated to include the new proxy service
- `Cargo.toml` - Workspace root; must add new crate to members

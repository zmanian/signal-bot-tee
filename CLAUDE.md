# Signal Bot TEE - Development Guide

## Security Architecture

### TEE Trust Model

This bot runs in an Intel TDX Trusted Execution Environment with the following security properties:

1. **Memory Protection**: All code and data in TEE memory is encrypted by the CPU
2. **Attestation**: Remote parties can verify the code running via TDX quotes
3. **Isolation**: The hypervisor/host cannot read TEE memory contents

### Why No Redis (Security Decision)

**Original Design**: Used Redis for conversation persistence.

**Problem**: Redis would store conversation history in plaintext. Even running inside the TEE:
- Redis data persisted to disk would be unencrypted
- The bot operator could read all conversations from Redis
- This completely breaks the privacy guarantee: "neither the bot operator nor NEAR AI can read your messages"

**Alternatives Considered**:
1. **Encrypt data with TEE-derived keys**: Complex, and TEE-derived keys may not survive restarts
2. **ORAM-based storage**: Protects access patterns, but overkill for this threat model since Signal metadata already leaks timing/who's messaging
3. **In-memory HashMap**: Simple, all data in TEE-protected memory, ephemeral

**Decision**: In-memory storage is the best fit because:
- All data stays in TEE-encrypted memory
- No external dependencies to secure
- Conversations naturally expire on restart (privacy feature)
- Simpler architecture = smaller attack surface
- ORAM unnecessary since Signal/NEAR AI network traffic already leaks comparable metadata

### Signal CLI Must Run in TEE (Critical)

Signal's E2E encryption terminates at the Signal CLI - that's where messages are decrypted.
**Both Signal CLI and the bot must run in the same TEE enclave.**

If Signal CLI runs outside the TEE:
```
User -> Signal servers -> [Signal CLI decrypts] -> plaintext -> TEE Bot
                              ^
                              |
                     Operator can read here (BROKEN!)
```

The operator could read decrypted messages from Signal CLI before they reach the bot -
completely breaking the privacy guarantee.

Correct architecture:
```
User -> Signal servers -> [TEE: Signal CLI decrypts -> Bot -> NEAR AI]
                          |_________________________________________|
                                   All in protected memory
```

The docker-compose deploys both `signal-api` and `signal-bot` containers in the same
Dstack TEE environment, ensuring plaintext only exists in protected memory.

### User Verification Process

Users can verify the bot is running securely in a TEE. This is critical - without verification,
you're trusting the operator's claim rather than cryptographic proof.

#### Step 1: Get Attestation with Challenge

Send `!verify <challenge>` to the bot, where `<challenge>` is any string you choose (like a random nonce or timestamp). This challenge proves the attestation was generated fresh for you, not replayed.

**Example**: `!verify my-random-nonce-12345`

**Challenge Handling**:
- If your challenge is **≤64 bytes**: It's embedded directly in the TDX quote's `report_data` field
- If your challenge is **>64 bytes**: It's hashed with SHA-256 first, then the hash is embedded

**Bot Response Includes**:
- **Your challenge**: Echo of what you sent
- **Whether it was hashed**: Indicates if SHA-256 was applied (for challenges >64 bytes)
- **Report data (hex)**: The exact data embedded in the TDX quote (your challenge or its hash)
- **Compose Hash**: Hash of the docker-compose.yaml running in the TEE
- **App ID**: Unique identifier for this TEE instance
- **TDX Quote (base64)**: The full hardware attestation quote
- **Verification Instructions**: Step-by-step guide to verify the quote

**Verifying Your Challenge**:
1. If your challenge was ≤64 bytes, convert it to hex and verify it matches the `report_data`
2. If your challenge was >64 bytes, hash it with SHA-256 and verify the hash matches `report_data`
3. This proves the attestation was generated in real-time for your specific request

**Example verification** (for challenge "test"):
```bash
# Your challenge in hex
echo -n "test" | xxd -p
# Output: 74657374

# Verify this appears at the start of report_data in the quote
# The bot shows: report_data = 74657374000000... (padded to 64 bytes)
```

#### Step 2: Verify the Compose Hash

The compose hash proves which exact containers are running. To verify:

1. Get the expected docker-compose.yaml from this repository
2. The Dstack attestation portal shows which compose hash is running
3. Verify they match

**Why this matters**: If an operator modified docker-compose.yaml (e.g., to run signal-api
outside the TEE), the compose hash would be different.

#### Step 3: Verify Image Pinning

Check that docker-compose.yaml pins signal-api to a specific digest, not `:latest`:

```yaml
# GOOD - Immutable, covered by compose_hash
image: bbernhard/signal-cli-rest-api@sha256:04ee57f9...

# BAD - Can change without changing compose_hash
image: bbernhard/signal-cli-rest-api:latest
```

The current pinned digest is in `docker/docker-compose.yaml`. You can verify this image
is the official one:

```bash
docker pull bbernhard/signal-cli-rest-api:latest
docker inspect --format='{{index .RepoDigests 0}}' bbernhard/signal-cli-rest-api:latest
# Should match the digest in docker-compose.yaml
```

#### Step 4: Verify on Phala Portal

Visit https://proof.phala.network and:
1. Enter the App ID from `!verify`
2. Confirm the TEE type is Intel TDX
3. Confirm the compose hash matches Step 2
4. Review the full attestation quote

#### What Attestation Proves

| Property | Verified By |
|----------|-------------|
| Code is running in Intel TDX | TDX hardware quote |
| Exact docker-compose.yaml | Compose hash in attestation |
| Signal CLI image version | Image digest in pinned compose |
| Memory is encrypted | TDX hardware guarantee |

#### What Attestation Does NOT Prove

- That the Signal CLI image itself is trustworthy (you trust bbernhard's image)
- Network-level metadata (timing, message sizes) is still visible to operator
- That NEAR AI is running in a TEE (verify separately at https://near.ai/verify)

#### Trust Summary

After verification, you trust:
1. **Intel TDX hardware** - CPU encrypts TEE memory
2. **Dstack/Phala** - Attestation infrastructure
3. **This repository's docker-compose.yaml** - Defines what runs in TEE
4. **bbernhard/signal-cli-rest-api** - The Signal CLI image
5. **NEAR AI** - Their GPU TEE for inference

### Data Flow Security

```
[TEE Boundary]
+------------------------------------------------------------------+
|  Signal CLI (decrypts) -> Bot (processes) -> NEAR AI request     |
|                              |                                    |
|                        [In-memory only]                          |
+------------------------------------------------------------------+
                               |
                               v
                         NEAR AI (GPU TEE)
```

- Signal E2E encryption terminates inside TEE
- Conversation history kept only in TEE memory
- Requests to NEAR AI go to their GPU TEE (attestable)
- No plaintext persistence anywhere
- Operator cannot access decrypted messages

### Metadata Leakage

The operator can still observe:
- Signal message arrival times and sizes (Signal CLI runs in TEE but metadata visible)
- NEAR AI request timing and sizes
- Which phone numbers are messaging

This is inherent to the architecture. ORAM would not significantly improve this since network-level metadata is the larger leak.

## Project Structure

```
crates/
  signal-bot/       # Main application
  near-ai-client/   # NEAR AI Cloud client with SecretString API keys
  conversation-store/  # In-memory HashMap with TTL
  dstack-client/    # TEE attestation via Dstack
  signal-client/    # Signal CLI REST API client
  signal-registration-proxy/  # Multi-tenant registration service
```

## Registration Proxy

### Purpose

The `signal-registration-proxy` crate enables multi-tenant Signal bot deployments where users can self-register their own phone numbers while preventing hijacking attacks.

**Problem solved**: The original architecture supports only a single pre-configured phone number. For a public TEE service, we need:
1. Self-service registration of new phone numbers
2. Prevention of re-registration attacks (once registered, only the original registrant can use it)
3. TEE-protected registration state that survives restarts

### Architecture

```
[External]                    [TEE Boundary]
    |                              |
    v                              v
+--------+    +--------------------+----------------------------+
| User   |--->| Registration Proxy | --> Signal CLI REST API    |
+--------+    |   (port 8081)      |        (port 8080)         |
              +--------------------+----------------------------+
                      |                        |
                      v                        v
              [Encrypted Registry]     [Signal Config Volume]
              (TEE-derived keys)       (account credentials)
```

### How It Works

#### 1. Phone Number Registry

The proxy maintains an encrypted registry of claimed phone numbers:

```rust
struct PhoneNumberRecord {
    phone_number: String,           // E.164 format (+14155551234)
    registered_at: DateTime<Utc>,   // When first registered
    status: RegistrationStatus,     // Pending | Verified | Failed
    ownership_proof_hash: Option<String>,  // SHA-256 of user's secret
}
```

#### 2. TEE-Encrypted Persistence

Unlike conversation storage (in-memory only), registration state must survive restarts:

1. **Key derivation**: Uses `DstackClient::derive_key("signal-registration-proxy/registry")` to derive a 32-byte AES key from the TEE root of trust
2. **Encryption**: AES-256-GCM with random 12-byte nonce
3. **Storage**: Encrypted file on Docker volume (`/data/registry.enc`)
4. **Atomic writes**: Temp file + rename to prevent corruption

**Security properties**:
- Same TEE deployment (same compose hash) always derives the same key
- Different deployment cannot decrypt old data (different compose hash = different key)
- File is encrypted at rest, safe even if volume accessed outside TEE

#### 3. Access Control

| Scenario | Allowed? | Reason |
|----------|----------|--------|
| Register new number | Yes | No prior claim |
| Re-register verified number | No | Prevents hijacking |
| Retry pending registration | Yes* | *Only with matching ownership_secret |
| Re-register failed attempt | Yes | Failed registrations can be retried |
| Unregister | Yes* | *Only with matching ownership_secret |

#### 4. Ownership Proof

Optional security feature to prevent hijacking of pending registrations:

1. User provides `ownership_secret` during initial registration
2. Proxy stores SHA-256 hash (never the plaintext)
3. Future operations (verify, unregister) require the same secret
4. Without ownership_secret, anyone with the verification code could complete registration

### HTTP API

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/register/{number}` | Initiate registration |
| `POST` | `/v1/register/{number}/verify/{code}` | Complete with SMS code |
| `GET` | `/v1/status/{number}` | Check registration status |
| `GET` | `/v1/accounts` | List all registered accounts |
| `DELETE` | `/v1/unregister/{number}` | Remove registration |
| `GET` | `/health` | Health check |

**Request body for registration**:
```json
{
  "captcha": "optional-captcha-token",
  "use_voice": false,
  "ownership_secret": "optional-secret-for-ownership-proof"
}
```

### Configuration

Environment variables for the registration proxy:

| Variable | Default | Description |
|----------|---------|-------------|
| `SIGNAL__API_URL` | `http://signal-api:8080` | Signal CLI REST API URL |
| `REGISTRY__PATH` | `/data/registry.enc` | Encrypted registry file path |
| `REGISTRY__PERSIST` | `true` | Enable persistence (false = in-memory only) |
| `SERVER__LISTEN_ADDR` | `0.0.0.0` | Listen address |
| `SERVER__PORT` | `8081` | Listen port |
| `DSTACK__SOCKET_PATH` | `/var/run/dstack.sock` | Dstack socket for TEE operations |
| `RATE_LIMIT__GLOBAL_PER_MINUTE` | `10` | Global rate limit |
| `RATE_LIMIT__PER_NUMBER_PER_HOUR` | `3` | Per-phone-number rate limit |

### Security Considerations

**Why encrypted persistence is OK here (but not for conversations)**:

| Data Type | Persistence | Reason |
|-----------|-------------|--------|
| Conversations | In-memory only | Privacy: conversations should be ephemeral |
| Registration state | Encrypted file | UX: users expect registered numbers to stay registered |

**Key insight**: Registration is about *claiming* a number, not about message content. The registry only stores phone numbers and timestamps, not conversation data.

**Rate limiting**: Prevents abuse of Signal's registration API and protects against enumeration attacks.

### Testing

```bash
# Run unit tests (includes encryption round-trip tests)
cargo test -p signal-registration-proxy

# Tests cover:
# - AES-256-GCM encryption/decryption
# - Tamper detection (authentication tag verification)
# - Registry serialization round-trip
# - Phone number normalization
# - Ownership proof verification
```

## Configuration

Environment variables (see `.env.example`):
- `SIGNAL__PHONE_NUMBER`: Bot's Signal phone number
- `NEAR_AI__API_KEY`: API key (stored as SecretString, never logged)
- `CONVERSATION__TTL`: How long conversations persist (default 24h)
- `CONVERSATION__MAX_MESSAGES`: Max messages per conversation (default 50)

## Testing

```bash
cargo test        # Run all tests
cargo build       # Build debug
cargo build --release  # Build release (with LTO)
```

## Deployment

### Prerequisites

- Docker and Docker Compose installed
- A phone number for Signal registration (prepaid SIM recommended)
- NEAR AI API key (get from https://near.ai)
- For TEE deployment: Phala/Dstack account and API key

### Step 1: Create Environment File

Create `docker/.env` with your secrets (this file is gitignored):

```bash
# Signal Configuration
SIGNAL_PHONE=+1YOURNUMBER

# NEAR AI Configuration
NEAR_AI_API_KEY=sk-your-api-key-here
NEAR_AI_BASE_URL=https://api.near.ai/v1
NEAR_AI_MODEL=llama-3.3-70b

# Conversation Settings
CONVERSATION_MAX_MESSAGES=50
CONVERSATION_TTL=24h

# Logging
LOG_LEVEL=info
```

### Step 2: Start Signal API (for registration)

```bash
cd docker
docker-compose up -d signal-api
```

Wait for healthy status:
```bash
docker-compose ps
# Should show: signal-api ... (healthy)
```

### Step 3: Register Signal Phone Number

Request verification code via SMS:
```bash
docker exec signal-api curl -s -X POST \
  "http://localhost:8080/v1/register/+1YOURNUMBER"
```

Or via voice call:
```bash
docker exec signal-api curl -s -X POST \
  "http://localhost:8080/v1/register/+1YOURNUMBER?voice=true"
```

Enter verification code when received:
```bash
docker exec signal-api curl -s -X POST \
  "http://localhost:8080/v1/register/+1YOURNUMBER/verify/CODE"
```

### Step 4: Set Up Signal Profile

```bash
docker exec signal-api curl -s -X PUT \
  "http://localhost:8080/v1/profiles/+1YOURNUMBER" \
  -H "Content-Type: application/json" \
  -d '{"name": "AI Assistant", "about": "TEE-secured AI assistant"}'
```

### Step 5: Start the Full Stack

```bash
docker-compose up -d
```

Monitor logs:
```bash
docker-compose logs -f signal-bot
```

### Step 6: Test the Bot

Send a message to your bot's Signal number. Try these commands:
- `!help` - Show available commands
- `!verify test123` - Get TEE attestation
- `!models` - List available AI models
- `!clear` - Clear conversation history
- Any other message - Chat with the AI

### Phala Cloud TEE Deployment

For production deployment to Phala's TEE infrastructure (Intel TDX):

#### Prerequisites

1. **Phala Cloud Account**: Sign up at https://cloud.phala.network
2. **Phala API Token**: Get from dashboard (P logo → API Tokens → Create Token)
3. **DockerHub Account**: To push the signal-bot image

#### Step 1: Install Phala CLI

```bash
npm install -g phala
phala --version  # Should show v1.0.x
```

#### Step 2: Authenticate with Phala Cloud

```bash
# Get API token from: cloud.phala.network → P logo → API Tokens → Create Token
phala auth login YOUR_API_TOKEN
phala auth status  # Verify authentication
```

**Note**: The API token format is different from `sk-rp-...` keys (those are registry keys).

#### Step 3: Push Bot Image to DockerHub

```bash
# Build the image
cd docker && docker-compose build signal-bot

# Tag for DockerHub
docker tag docker-signal-bot:latest YOUR_DOCKERHUB/signal-bot-tee:latest

# Login and push
docker login
docker push YOUR_DOCKERHUB/signal-bot-tee:latest
```

#### Step 4: Update Phala Compose File

Edit `docker/phala-compose.yaml` and set your image:

```yaml
signal-bot:
  image: YOUR_DOCKERHUB/signal-bot-tee:latest
```

#### Step 5: Deploy to Phala Cloud

**Option A: Via CLI**

```bash
cd docker
phala cvms create \
  --name signal-bot-tee \
  --compose ./phala-compose.yaml \
  --vcpu 2 \
  --memory 4096 \
  --disk-size 20
```

When prompted, enter your encrypted secrets:
- `SIGNAL_PHONE`: Your Signal phone number (e.g., +16504928286)
- `NEAR_AI_API_KEY`: Your NEAR AI API key
- `NEAR_AI_BASE_URL`: https://cloud-api.near.ai/v1
- `NEAR_AI_MODEL`: deepseek-ai/DeepSeek-V3.1

**Option B: Via Dashboard**

1. Go to https://cloud.phala.network/dashboard/cvm
2. Click "Deploy" → "From Docker Compose"
3. Upload `docker/phala-compose.yaml`
4. Configure encrypted secrets in the UI
5. Select TEE type: Intel TDX
6. Click Deploy

#### Step 6: Verify Deployment

```bash
# List your CVMs
phala cvms list

# Get details (including endpoint URL)
phala cvms get APP_ID

# Check attestation
phala cvms attestation APP_ID
```

Your bot endpoint will be:
`https://[app-id]-8080.dstack-prod5.phala.network`

#### Step 7: Transfer Signal Registration

**Important**: Signal registration is stored in a Docker volume. You need to either:

1. **Re-register in TEE**: Run Signal registration commands via the TEE deployment
2. **Export/Import**: Copy registration data from local to TEE volume

To re-register in TEE:
```bash
# Get your CVM's shell access from Phala dashboard
# Then run Signal registration as described above
```

#### Encrypted Secrets Reference

Phala Cloud encrypts these at rest and decrypts only inside the TEE:

| Variable | Description |
|----------|-------------|
| `SIGNAL_PHONE` | Bot's Signal phone number |
| `NEAR_AI_API_KEY` | NEAR AI Cloud API key |
| `NEAR_AI_BASE_URL` | https://cloud-api.near.ai/v1 |
| `NEAR_AI_MODEL` | AI model (default: deepseek-ai/DeepSeek-V3.1) |

#### Troubleshooting Phala Deployment

**"Invalid API key"**: Make sure you're using an API Token from the dashboard (P logo → API Tokens), not an `sk-rp-...` registry key.

**Image pull fails**: Ensure your DockerHub image is public, or configure private registry credentials in Phala.

**No attestation**: The dstack socket should be auto-mounted at `/var/run/dstack.sock` in Phala CVMs.

### Security Checklist

- [ ] `docker/.env` contains secrets and is NOT committed to git
- [ ] `docker-compose.yaml` does NOT expose port 8080 publicly
- [ ] Signal API is only accessible within Docker internal network
- [ ] Signal CLI image is pinned to specific SHA256 digest
- [ ] NEAR AI API key is set and working
- [ ] TEE attestation works (test with `!verify`)

### Troubleshooting

**Signal registration fails**:
- VoIP numbers often blocked; use real carrier SIM
- Wait 24h if rate limited

**Bot not responding**:
- Check logs: `docker-compose logs signal-bot`
- Verify NEAR AI key is valid
- Ensure Signal number matches `.env`

**No TEE attestation**:
- Running locally (not in Dstack) is expected - attestation only works in TEE
- In TEE, check `/var/run/dstack.sock` exists

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

Visit https://proof.t16z.com and:
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
- That NEAR AI is running in a TEE (verify separately at https://docs.near.ai/cloud/verification/)

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
  tools/            # Tool use system (calculator, weather, web search)
web/                # React frontend (Vite + Tailwind, deployed to Vercel)
docker/             # Docker Compose configs for local and Phala deployment
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

### Tool Configuration

Environment variables for the tool use system:

| Variable | Default | Description |
|----------|---------|-------------|
| `TOOLS__ENABLED` | `true` | Master switch for tool system |
| `TOOLS__MAX_TOOL_CALLS` | `5` | Max tool executions per message |
| `TOOLS__CALCULATOR__ENABLED` | `true` | Enable calculator tool |
| `TOOLS__WEATHER__ENABLED` | `true` | Enable weather tool |
| `TOOLS__WEB_SEARCH__ENABLED` | `true` | Enable web search tool |
| `TOOLS__WEB_SEARCH__API_KEY` | (none) | Brave Search API key |
| `TOOLS__WEB_SEARCH__MAX_RESULTS` | `5` | Number of search results |

## Tool Use System

The bot supports LLM tool use (function calling) for enhanced capabilities:

### Available Tools

| Tool | Description | API Key Required? |
|------|-------------|-------------------|
| `calculate` | Evaluate math expressions (uses `meval` crate) | No |
| `get_weather` | Current weather for any location (Open-Meteo API) | No |
| `web_search` | Search the web for current information (Brave Search) | Yes |

### How Tools Work

1. User sends a message that might benefit from a tool (e.g., "What's 2^10?" or "Weather in Tokyo")
2. The LLM decides to call one or more tools and returns a tool_calls response
3. Bot sends a progress message to user: "🔧 Using calculate..."
4. Bot executes the tool and gets results
5. Results are added to conversation and sent back to LLM
6. LLM formulates a natural language response incorporating tool results
7. Bot sends final response to user

This loop can repeat up to `TOOLS__MAX_TOOL_CALLS` times per user message.

### Setting Up Brave Search API

Web search requires a Brave Search API key:

1. Go to https://brave.com/search/api/
2. Click "Get Started for Free"
3. Create account and verify email
4. Generate API key from dashboard
5. Free tier: 2,000 queries/month

Set the environment variable:
```bash
TOOLS__WEB_SEARCH__API_KEY=your-brave-api-key
```

### Disabling Tools

To run without tools:
```bash
TOOLS__ENABLED=false
```

To disable specific tools:
```bash
TOOLS__WEB_SEARCH__ENABLED=false
TOOLS__WEATHER__ENABLED=false
```

### Architecture

```
crates/tools/
├── src/
│   ├── lib.rs           # Module exports
│   ├── types.rs         # ToolDefinition, ToolCall, ToolResult, Tool trait
│   ├── registry.rs      # ToolRegistry - manages available tools
│   ├── executor.rs      # ToolExecutor - timeout, error handling
│   ├── error.rs         # ToolError enum
│   └── builtin/
│       ├── calculator.rs   # Pure Rust math (meval crate)
│       ├── weather.rs      # Open-Meteo API (free, no key)
│       └── web_search.rs   # Brave Search API
```

The tool system uses OpenAI-compatible function calling schema, which NEAR AI supports.

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

#### Step 3: Build and Push Images to DockerHub

**IMPORTANT**: Phala Cloud runs on linux/amd64. You MUST build images for this platform:

```bash
cd /path/to/signal-bot-tee

# Build signal-bot for linux/amd64
docker buildx build --platform linux/amd64 \
  -t YOUR_DOCKERHUB/signal-bot-tee:latest \
  -f docker/Dockerfile --push .

# Build signal-registration-proxy for linux/amd64
docker buildx build --platform linux/amd64 \
  -t YOUR_DOCKERHUB/signal-registration-proxy:vX.Y.Z \
  -f docker/Dockerfile.proxy --push .
```

**Why this matters**: If you build without `--platform linux/amd64` on an ARM Mac (M1/M2/M3), Docker builds for arm64 by default. Phala CVMs cannot run arm64 images and will fail with "no matching manifest for linux/amd64".

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

#### Updating the Deployed Image

Phala CVMs cache Docker images. To deploy a new version of signal-bot:

**Option A: Upgrade Existing CVM (Recommended)**

```bash
# Build and push new image
cd docker && docker-compose build signal-bot
docker tag docker-signal-bot:latest zaki1iqlusion/signal-bot-tee:latest
docker push zaki1iqlusion/signal-bot-tee:latest

# Upgrade the CVM (pulls fresh image)
phala deploy --uuid APP_ID --compose ./phala-compose.yaml
```

The `--uuid` flag upgrades an existing CVM rather than creating a new one.

**Option B: Delete and Recreate**

If upgrade fails or you need a completely fresh deployment:

```bash
# Delete old CVM
phala cvms delete APP_ID

# Deploy fresh
phala deploy --name signal-bot-tee --compose ./phala-compose.yaml \
  --vcpu 2 --memory 4096 --disk-size 20
```

**Note**: This creates a new CVM with a new App ID. Signal registration data is preserved in the volume.

**Option C: Force Image Pull (Dashboard)**

1. Go to https://cloud.phala.network/dashboard/cvm
2. Select your CVM
3. Click "Upgrade" → "Force Pull Images"
4. This restarts containers with fresh image pulls

#### Troubleshooting Phala Deployment

**"Invalid API key"**: Make sure you're using an API Token from the dashboard (P logo → API Tokens), not an `sk-rp-...` registry key.

**Image pull fails**: Ensure your DockerHub image is public, or configure private registry credentials in Phala.

**No attestation**: The dstack socket should be auto-mounted at `/var/run/dstack.sock` in Phala CVMs.

**CVM restart/stop CLI errors**: The Phala CLI sometimes has API compatibility issues. Use the dashboard at https://cloud.phala.network/dashboard/cvm for restart/stop operations if CLI fails.

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

### Debugging Signal Registration Issues

If registration via the proxy returns success but no SMS/call is received, follow this debugging process:

#### Step 1: Check Proxy Debug Endpoints

The registration proxy has debug endpoints to inspect Signal CLI state:

```bash
# List accounts Signal CLI knows about (not our proxy registry)
curl https://YOUR_ENDPOINT-8081.dstack-pha-prod9.phala.network/v1/debug/signal-accounts

# Force unregister from Signal CLI (bypasses proxy registry check)
curl -X POST https://YOUR_ENDPOINT-8081.dstack-pha-prod9.phala.network/v1/debug/force-unregister/+1YOURNUMBER
```

#### Step 2: Expose Signal CLI Directly (Temporary)

If the proxy seems to work but registration still fails, expose Signal CLI port 8080 temporarily to bypass the proxy:

```yaml
# In phala-compose.yaml, add to signal-api service:
ports:
  - "8080:8080"
```

Deploy the change, then test directly:

```bash
# Check Signal CLI health
curl https://YOUR_ENDPOINT-8080.dstack-pha-prod9.phala.network/v1/health

# List accounts directly
curl https://YOUR_ENDPOINT-8080.dstack-pha-prod9.phala.network/v1/accounts

# Register directly (with captcha from signalcaptchas.org/registration/generate.html)
curl -X POST https://YOUR_ENDPOINT-8080.dstack-pha-prod9.phala.network/v1/register/+1YOURNUMBER \
  -H "Content-Type: application/json" \
  -d '{"captcha": "signalcaptcha://..."}'
```

#### Step 3: Common Errors and Solutions

| Error | Cause | Solution |
|-------|-------|----------|
| `"Account is already registered (IOException)"` | Stale data in `signal-config` volume | Rename volume to force fresh start (e.g., `signal-config-v2`) |
| `"java.net.SocketTimeoutException: timeout"` | Network issue or rate limiting | Retry; may be transient |
| `"Captcha required"` | Signal requires captcha | Get token from signalcaptchas.org/registration/generate.html |
| HTTP 201 but no SMS | Success! Code was sent | Check phone; try voice if SMS blocked |
| HTTP 400 with captcha error | Captcha expired | Captchas expire quickly; get a fresh one |
| `"[403] Authorization failed"` | Captcha expired or invalid | Get a fresh captcha immediately before registering |
| `"[429] Rate Limited"` | Too many registration attempts | Wait 24 hours before retrying |
| Signal API container crash loop | Invalid `AUTO_RECEIVE_SCHEDULE` format | Use 5-field cron format (e.g., `* * * * *`), not 6-field |
| `signal_api_healthy: false` | Signal CLI container not running | Check if container is crashing; review cron format |

#### Step 4: Stale Volume Fix

The most common issue is **stale data in the Signal CLI volume**. Signal CLI stores registration state locally, and corrupt/partial data causes `IOException` errors.

**Symptoms**:
- Proxy returns success but no SMS arrives
- Direct Signal CLI calls return `"Account is already registered (IOException)"`
- `curl /v1/accounts` shows the number even after "unregistering"

**Solution**: Force a fresh volume by renaming it:

```yaml
# Change in phala-compose.yaml:
volumes:
  - signal-config-v2:/home/.local/share/signal-cli  # was signal-config

# Also update volumes section:
volumes:
  signal-config-v2:
    driver: local
```

Then redeploy. This gives Signal CLI a clean data directory.

#### Step 5: After Debugging

**Important**: Remove the debug port exposure after fixing:

```yaml
# Remove from signal-api service:
ports:
  - "8080:8080"
```

This keeps Signal CLI only accessible internally via the proxy, which is the secure configuration.

## Web Frontend

The project includes a web frontend for TEE verification and bot discovery.

### Tech Stack

- **Framework**: React 19 + Vite 7
- **Language**: TypeScript
- **Styling**: Tailwind CSS v4
- **Data Fetching**: TanStack Query (React Query)
- **Animations**: Framer Motion
- **Icons**: Lucide React

### Project Structure

```
web/
├── src/
│   ├── App.tsx              # Main app component
│   ├── main.tsx             # Entry point
│   ├── components/
│   │   ├── Hero.tsx         # Landing hero section
│   │   ├── BotCard.tsx      # Bot display card
│   │   ├── RegistrationForm.tsx  # Phone number registration
│   │   └── VerificationPanel.tsx  # TEE attestation UI
│   ├── hooks/
│   │   ├── useBots.ts       # Bot list fetching
│   │   └── useAttestation.ts  # TEE attestation fetching
│   └── lib/
│       └── api.ts           # API client
├── vercel.json              # Vercel deployment config
├── package.json
└── tsconfig.json
```

### Local Development

```bash
cd web
npm install
npm run dev      # Start dev server at http://localhost:5173
npm run build    # Build for production
npm run preview  # Preview production build
```

### Vercel Deployment

The frontend is deployed to Vercel at: https://signal-tee-web.vercel.app

#### Configuration

`web/vercel.json`:
```json
{
  "framework": "vite",
  "buildCommand": "npm run build",
  "outputDirectory": "dist",
  "env": {
    "VITE_API_URL": "https://[app-id]-8081.dstack-pha-prod9.phala.network"
  }
}
```

#### Deploying to Vercel

**Option A: Via Vercel CLI**

```bash
cd web
npm install -g vercel
vercel login
vercel          # Deploy preview
vercel --prod   # Deploy to production
```

**Option B: Via GitHub Integration (Recommended)**

1. Push your code to GitHub
2. Go to https://vercel.com/new
3. Import the repository
4. Set root directory to `web`
5. Vercel auto-detects Vite framework
6. Add environment variable `VITE_API_URL` pointing to your Phala deployment
7. Deploy

#### Environment Variables

| Variable | Description |
|----------|-------------|
| `VITE_API_URL` | Registration proxy URL (Phala deployment) |

**Updating the API URL**: When you redeploy the backend to a new Phala CVM, update `VITE_API_URL` in:
1. `web/vercel.json` (for new deployments)
2. Vercel dashboard → Project Settings → Environment Variables (for existing deployment)

Then redeploy the frontend:
```bash
cd web && vercel --prod
```

#### Production URL

After deployment, Vercel provides a URL like:
- `https://signal-tee-web.vercel.app` (custom domain or default)
- `https://signal-tee-web-[team].vercel.app` (team URL)

The frontend connects to the Phala-deployed registration proxy to:
- List registered Signal bot accounts
- Fetch TEE attestation data for verification
- Display bot status and health

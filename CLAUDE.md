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

Deploy via Docker in Dstack TEE environment:
```bash
cd docker && docker-compose up -d
```

The bot requires:
- Dstack socket at `/var/run/dstack.sock` for attestation
- Signal CLI REST API (included in docker-compose)
- Network access to NEAR AI Cloud

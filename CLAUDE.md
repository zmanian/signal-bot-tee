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

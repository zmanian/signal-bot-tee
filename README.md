# Signal Bot TEE

Private AI Chat Proxy running in Trusted Execution Environment (TEE)

## Overview

This project implements a Signal bot that runs inside a Dstack-powered TEE (Intel TDX) and proxies user messages to NEAR AI Cloud's private inference API. The design creates a fully verifiable, end-to-end private AI chat experience.

## Architecture

```
[User] <--Signal E2E--> [TEE: Signal CLI + Bot] <--HTTPS--> [NEAR AI GPU TEE]
                              |
                        [In-memory only]
                        [Intel TDX protected]
```

- **Signal**: E2E encrypted messaging between user and bot
- **Dstack TEE**: Verifiable proxy execution with Intel TDX attestation
- **NEAR AI Cloud**: Private inference with GPU TEE (NVIDIA H100/H200) attestation

## Features

- End-to-end privacy from user device to AI inference
- Dual attestation (Intel TDX + NVIDIA GPU TEE)
- Cryptographic verification with user-provided challenges
- In-memory conversation storage (no external persistence)
- OpenAI-compatible API integration

## Bot Commands

| Command | Description |
|---------|-------------|
| `!verify <challenge>` | Get TEE attestation with your challenge embedded in TDX quote |
| `!clear` | Clear conversation history |
| `!models` | List available AI models |
| `!help` | Show help message |

Any other message is sent to the AI for a response.

## Verification

Users can cryptographically verify the bot runs in a genuine TEE:

1. Send `!verify my-random-nonce` to the bot
2. Bot returns a TDX quote with your nonce embedded in `report_data`
3. Verify the quote signature at https://proof.phala.network
4. Compare `compose_hash` with this repository's `docker/docker-compose.yaml`

This proves:
- The attestation was generated fresh (contains your nonce)
- The code is running in Intel TDX hardware
- The exact docker-compose configuration is as published

## Project Structure

```
crates/
  signal-bot/          # Main application binary
  near-ai-client/      # NEAR AI Cloud API client
  conversation-store/  # In-memory conversation storage with TTL
  dstack-client/       # Dstack TEE attestation client
  signal-client/       # Signal CLI REST API client
docker/
  Dockerfile           # Multi-stage build for Alpine
  docker-compose.yaml  # Production deployment config
```

## Security Model

See [CLAUDE.md](./CLAUDE.md) for detailed security documentation including:

- Why in-memory storage instead of Redis
- Why Signal CLI must run inside the TEE
- User verification process
- Trust assumptions and metadata leakage

## Quick Start

### Prerequisites

- Rust 1.83+
- Docker & Docker Compose
- Signal phone number (for the bot)
- NEAR AI API key

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
```

### Deploy

```bash
cd docker
cp ../.env.example .env
# Edit .env with your credentials
docker-compose up -d
```

## Configuration

Environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `SIGNAL__PHONE_NUMBER` | Bot's Signal phone number | Required |
| `SIGNAL__SERVICE_URL` | Signal CLI REST API URL | `http://signal-api:8080` |
| `NEAR_AI__API_KEY` | NEAR AI API key | Required |
| `NEAR_AI__MODEL` | AI model to use | `llama-3.3-70b` |
| `CONVERSATION__TTL` | Conversation expiry time | `24h` |
| `CONVERSATION__MAX_MESSAGES` | Max messages per conversation | `50` |

## License

MIT

# Signal Bot TEE

Private AI Chat Proxy running in Trusted Execution Environment (TEE)

## Overview

This project implements a Signal bot that runs inside a Dstack-powered TEE (Intel TDX) and proxies user messages to NEAR AI Cloud's private inference API. The design creates a fully verifiable, end-to-end private AI chat experience.

## Architecture

- **Signal**: E2E encrypted messaging between user and bot
- **Dstack TEE**: Verifiable proxy execution with Intel TDX attestation
- **NEAR AI Cloud**: Private inference with GPU TEE (NVIDIA H100/H200) attestation

## Documentation

See [DESIGN.md](./DESIGN.md) for the complete technical design document.

## Features

- ✅ End-to-end privacy from user device to AI inference
- ✅ Dual attestation (Intel TDX + NVIDIA GPU TEE)
- ✅ OpenAI-compatible API integration
- ✅ Verifiable code execution
- ✅ Conversation history management

## Status

🚧 Design phase - Implementation in progress

## License

MIT

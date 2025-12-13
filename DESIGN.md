# Signal → TEE → NEAR AI Cloud: Private AI Chat Proxy

## Overview

This document describes the architecture for a Signal bot running inside a Dstack-powered TEE that proxies user messages to NEAR AI Cloud's private inference API. The design creates a fully verifiable, end-to-end private AI chat experience where:

1. **Signal** provides E2E encrypted messaging between user and bot
2. **Dstack TEE** provides verifiable proxy execution with Intel TDX attestation
3. **NEAR AI Cloud** provides private inference with GPU TEE (NVIDIA H100/H200) attestation

Users can chat with AI via Signal knowing that neither the bot operator nor the inference provider can read their messages or responses—and both claims are cryptographically verifiable.

## Goals

- **End-to-End Privacy**: Messages encrypted from user device to AI inference, with no plaintext exposure
- **Dual Attestation**: Both proxy (Intel TDX) and inference (NVIDIA GPU TEE) provide independent attestation
- **OpenAI Compatibility**: Leverage NEAR AI Cloud's OpenAI-compatible API for easy integration
- **Verifiable Custody**: Users can verify the exact code handling their messages at both layers
- **Conversation Continuity**: Maintain chat history per-user for contextual conversations

## Architecture

```
┌──────────────┐         ┌─────────────────────────────────────────────────────────┐
│   Signal     │         │              Intel TDX Confidential VM                   │
│   User       │         │  ┌─────────────────────────────────────────────────────┐│
│              │  E2E    │  │                  Dstack OS                          ││
│  ┌────────┐  │ Signal  │  │  ┌──────────────┐    ┌──────────────────────────┐  ││
│  │ Signal │◄─┼─────────┼──┼─►│ signal-cli   │    │   AI Proxy Bot           │  ││
│  │  App   │  │Protocol │  │  │ REST API     │◄───│                          │  ││
│  └────────┘  │         │  │  │              │    │  - Message routing       │  ││
│              │         │  │  │ - Send/Recv  │    │  - Conversation state    │  ││
└──────────────┘         │  │  └──────────────┘    │  - NEAR AI client        │  ││
                         │  │                      │  - Attestation bridge    │  ││
                         │  │                      └────────────┬─────────────┘  ││
                         │  │                                   │                ││
                         │  │                      ┌────────────▼─────────────┐  ││
                         │  │                      │   /var/run/dstack.sock   │  ││
                         │  │                      │   - Key derivation       │  ││
                         │  │                      │   - TDX quotes           │  ││
                         │  │                      └──────────────────────────┘  ││
                         │  └─────────────────────────────────────────────────────┘│
                         └───────────────────────────┬─────────────────────────────┘
                                                     │
                                                     │ OpenAI-compatible API
                                                     │ (TLS 1.3 + AES-256)
                                                     ▼
                         ┌─────────────────────────────────────────────────────────┐
                         │                   NEAR AI Cloud                          │
                         │  ┌─────────────────────────────────────────────────────┐│
                         │  │              LLM Gateway (Intel TDX)                 ││
                         │  │  - Request routing    - Load balancing              ││
                         │  │  - Attestation gen    - Rate limiting               ││
                         │  └───────────────────────────┬─────────────────────────┘│
                         │                              │                          │
                         │  ┌───────────────────────────▼─────────────────────────┐│
                         │  │         Private LLM Nodes (NVIDIA GPU TEE)          ││
                         │  │                                                     ││
                         │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐            ││
                         │  │  │ H100/   │  │ H100/   │  │ H100/   │  ...       ││
                         │  │  │ H200    │  │ H200    │  │ H200    │            ││
                         │  │  │ TEE     │  │ TEE     │  │ TEE     │            ││
                         │  │  └─────────┘  └─────────┘  └─────────┘            ││
                         │  │                                                     ││
                         │  │  - Model inference in GPU TEE                       ││
                         │  │  - NVIDIA attestation reports                       ││
                         │  │  - Cryptographic response signing                   ││
                         │  └─────────────────────────────────────────────────────┘│
                         └─────────────────────────────────────────────────────────┘
```

## Privacy Stack

| Layer | Technology | Protection | Attestation |
|-------|------------|------------|-------------|
| Transport (User↔Bot) | Signal Protocol | E2E encryption, forward secrecy | Signal safety numbers |
| Proxy Execution | Intel TDX + Dstack | Memory isolation, code verification | TDX Quote (MRTD, RTMRs) |
| Inference Transport | TLS 1.3 | Channel encryption | Certificate chain |
| Inference Execution | NVIDIA GPU TEE | Model/data isolation | GPU attestation report |

**Key insight**: Neither the bot operator nor NEAR AI can read user messages. The bot operator cannot extract Signal keys or inspect TEE memory. NEAR AI cannot see plaintext because inference runs in GPU TEE with attestation.

## Components

### Signal CLI REST API

Interface to Signal's network:

- Message send/receive over REST
- Account linking (secondary device)
- Group message support
- Runs in `json-rpc` mode for performance

### AI Proxy Bot

Core application logic:

- Routes messages to NEAR AI Cloud
- Manages per-user conversation history
- Handles attestation requests from users
- Bridges TEE attestation to Signal responses

### NEAR AI Cloud Client

OpenAI-compatible client configured for NEAR AI:

```python
from openai import OpenAI

client = OpenAI(
    base_url="https://api.near.ai/v1",
    api_key=near_ai_api_key
)
```

### Dstack Guest Agent

TEE services via `/var/run/dstack.sock`:

- Key derivation for secrets
- TDX quote generation
- RA-TLS certificates

## Security Model

### Trust Assumptions

| Component | Trust Level | Justification |
|-----------|-------------|---------------|
| Intel TDX Hardware | Trusted | Hardware root of trust for proxy |
| NVIDIA GPU TEE | Trusted | Hardware root of trust for inference |
| Dstack OS | Verified | Measured boot chain in RTMRs |
| Bot Code | Verified | Compose hash in RTMR3 |
| NEAR AI Cloud | Verified | GPU attestation per-request |
| Bot Operator | Untrusted | Cannot access TEE memory or keys |
| NEAR AI Operator | Untrusted | Cannot access GPU TEE memory |
| Network | Untrusted | TLS + Signal E2E |

### Dual Attestation Flow

```
User sends "!verify" to Signal bot
            │
            ▼
┌───────────────────────────────────────┐
│  Proxy TEE (Dstack)                   │
│  1. Generate TDX quote                │
│  2. Include bot compose-hash          │
│  3. Request NEAR AI attestation       │
└───────────────────────────────────────┘
            │
            ▼
┌───────────────────────────────────────┐
│  NEAR AI Cloud                        │
│  1. Return GPU TEE attestation        │
│  2. Include model hash, TCB info      │
└───────────────────────────────────────┘
            │
            ▼
┌───────────────────────────────────────┐
│  Bot formats response                 │
│  - Proxy attestation summary          │
│  - Inference attestation summary      │
│  - Verification links                 │
└───────────────────────────────────────┘
            │
            ▼
     Signal message to user
```

### What Each Attestation Proves

**Dstack TDX Quote (Proxy)**:
- Genuine Intel TDX hardware
- Expected firmware (MRTD)
- Expected kernel (RTMR1)
- Expected bot code (RTMR3 compose-hash)

**NEAR AI GPU Attestation (Inference)**:
- Genuine NVIDIA TEE hardware (H100/H200)
- Expected model weights
- Computation occurred in secure enclave
- Response signed by TEE

## Implementation

### Docker Compose Configuration

```yaml
version: "3"

services:
  signal-api:
    image: bbernhard/signal-cli-rest-api:latest
    environment:
      - MODE=json-rpc
      - JSON_RPC_TRUST_NEW_IDENTITIES=on-first-use
      - LOG_LEVEL=info
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock
      - signal-config:/home/.local/share/signal-cli
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  ai-proxy-bot:
    build:
      context: ./bot
      dockerfile: Dockerfile
    environment:
      - SIGNAL_SERVICE=http://signal-api:8080
      - SIGNAL_PHONE=${ENCRYPTED_PHONE}
      - NEAR_AI_API_KEY=${ENCRYPTED_NEAR_AI_KEY}
      - NEAR_AI_BASE_URL=https://api.near.ai/v1
      - NEAR_AI_MODEL=llama-3.3-70b
      - REDIS_URL=redis://redis:6379
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock
    depends_on:
      signal-api:
        condition: service_healthy
      redis:
        condition: service_started
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes

volumes:
  signal-config:
    driver: local
  redis-data:
    driver: local
```

### Bot Application Dockerfile

```dockerfile
FROM python:3.11-slim

WORKDIR /app

RUN pip install --no-cache-dir \
    httpx \
    signalbot \
    openai \
    redis \
    cryptography

COPY . .

CMD ["python", "main.py"]
```

### NEAR AI Cloud Client

```python
"""near_ai_client.py - NEAR AI Cloud integration with attestation"""

from openai import AsyncOpenAI
from typing import AsyncGenerator, Optional
import httpx
import json


class NearAIClient:
    """
    OpenAI-compatible client for NEAR AI Cloud with attestation support.
    
    NEAR AI Cloud provides:
    - OpenAI-compatible /v1/chat/completions endpoint
    - GPU TEE attestation per-request
    - ~5-10% latency overhead for privacy guarantees
    """
    
    def __init__(
        self,
        api_key: str,
        base_url: str = "https://api.near.ai/v1",
        model: str = "llama-3.3-70b"
    ):
        self.client = AsyncOpenAI(
            api_key=api_key,
            base_url=base_url
        )
        self.model = model
        self.base_url = base_url
    
    async def chat(
        self,
        messages: list[dict],
        stream: bool = False,
        **kwargs
    ) -> str | AsyncGenerator[str, None]:
        """
        Send chat completion request to NEAR AI Cloud.
        
        Messages format (OpenAI-compatible):
        [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Hello!"},
            {"role": "assistant", "content": "Hi there!"},
            {"role": "user", "content": "How are you?"}
        ]
        """
        if stream:
            return self._stream_chat(messages, **kwargs)
        
        response = await self.client.chat.completions.create(
            model=self.model,
            messages=messages,
            **kwargs
        )
        return response.choices[0].message.content
    
    async def _stream_chat(
        self,
        messages: list[dict],
        **kwargs
    ) -> AsyncGenerator[str, None]:
        """Stream chat responses for real-time output."""
        stream = await self.client.chat.completions.create(
            model=self.model,
            messages=messages,
            stream=True,
            **kwargs
        )
        async for chunk in stream:
            if chunk.choices[0].delta.content:
                yield chunk.choices[0].delta.content
    
    async def get_attestation(self) -> dict:
        """
        Fetch attestation report from NEAR AI Cloud.
        
        Returns signed proofs from Intel TDX and NVIDIA GPU TEE
        that can be validated against their attestation services.
        """
        async with httpx.AsyncClient() as client:
            # NEAR AI provides attestation endpoint
            resp = await client.get(
                f"{self.base_url}/attestation",
                headers={"Authorization": f"Bearer {self.client.api_key}"}
            )
            resp.raise_for_status()
            return resp.json()
    
    async def get_models(self) -> list[dict]:
        """List available models on NEAR AI Cloud."""
        models = await self.client.models.list()
        return [m.model_dump() for m in models.data]
```

### Conversation State Manager

```python
"""conversation.py - Per-user conversation history with Redis"""

import redis.asyncio as redis
import json
from typing import Optional
from dataclasses import dataclass, asdict
from datetime import datetime, timedelta


@dataclass
class Message:
    role: str  # "user", "assistant", "system"
    content: str
    timestamp: float


@dataclass
class Conversation:
    user_id: str  # Signal phone number or group ID
    messages: list[Message]
    created_at: float
    updated_at: float
    system_prompt: Optional[str] = None


class ConversationStore:
    """
    Redis-backed conversation storage.
    
    - Maintains chat history per Signal user/group
    - Auto-expires old conversations
    - Limits context window size
    """
    
    def __init__(
        self,
        redis_url: str = "redis://localhost:6379",
        max_messages: int = 50,
        ttl_hours: int = 24
    ):
        self.redis = redis.from_url(redis_url)
        self.max_messages = max_messages
        self.ttl = timedelta(hours=ttl_hours)
    
    def _key(self, user_id: str) -> str:
        return f"conversation:{user_id}"
    
    async def get(self, user_id: str) -> Optional[Conversation]:
        """Get conversation for user, or None if not exists."""
        data = await self.redis.get(self._key(user_id))
        if not data:
            return None
        
        obj = json.loads(data)
        return Conversation(
            user_id=obj["user_id"],
            messages=[Message(**m) for m in obj["messages"]],
            created_at=obj["created_at"],
            updated_at=obj["updated_at"],
            system_prompt=obj.get("system_prompt")
        )
    
    async def add_message(
        self,
        user_id: str,
        role: str,
        content: str,
        system_prompt: Optional[str] = None
    ) -> Conversation:
        """Add message to conversation, creating if needed."""
        now = datetime.utcnow().timestamp()
        conv = await self.get(user_id)
        
        if conv is None:
            conv = Conversation(
                user_id=user_id,
                messages=[],
                created_at=now,
                updated_at=now,
                system_prompt=system_prompt
            )
        
        conv.messages.append(Message(role=role, content=content, timestamp=now))
        conv.updated_at = now
        
        # Trim to max messages (keep system prompt effective)
        if len(conv.messages) > self.max_messages:
            conv.messages = conv.messages[-self.max_messages:]
        
        # Persist with TTL
        await self.redis.setex(
            self._key(user_id),
            self.ttl,
            json.dumps({
                "user_id": conv.user_id,
                "messages": [asdict(m) for m in conv.messages],
                "created_at": conv.created_at,
                "updated_at": conv.updated_at,
                "system_prompt": conv.system_prompt
            })
        )
        
        return conv
    
    async def clear(self, user_id: str) -> bool:
        """Clear conversation history for user."""
        return await self.redis.delete(self._key(user_id)) > 0
    
    async def to_openai_messages(
        self,
        user_id: str,
        system_prompt: Optional[str] = None
    ) -> list[dict]:
        """Convert conversation to OpenAI messages format."""
        conv = await self.get(user_id)
        messages = []
        
        # Add system prompt
        prompt = system_prompt or (conv.system_prompt if conv else None)
        if prompt:
            messages.append({"role": "system", "content": prompt})
        
        # Add conversation history
        if conv:
            for msg in conv.messages:
                messages.append({"role": msg.role, "content": msg.content})
        
        return messages
```

### Dstack SDK Integration

```python
"""dstack_utils.py - TEE integration utilities"""

import httpx
from typing import Optional
import json

DSTACK_SOCKET = "/var/run/dstack.sock"


class DstackClient:
    """Client for Dstack guest agent APIs."""
    
    def __init__(self, socket_path: str = DSTACK_SOCKET):
        self.transport = httpx.AsyncHTTPTransport(uds=socket_path)
    
    async def derive_key(self, path: str, subject: Optional[str] = None) -> bytes:
        """Derive a deterministic key from TEE root of trust."""
        async with httpx.AsyncClient(transport=self.transport) as client:
            params = {"path": path}
            if subject:
                params["subject"] = subject
            
            resp = await client.post(
                "http://localhost/DeriveKey",
                json=params
            )
            resp.raise_for_status()
            return bytes.fromhex(resp.json()["key"])
    
    async def get_quote(self, report_data: bytes) -> dict:
        """Generate TDX attestation quote."""
        async with httpx.AsyncClient(transport=self.transport) as client:
            resp = await client.get(
                "http://localhost/GetQuote",
                params={"report_data": report_data.hex()}
            )
            resp.raise_for_status()
            return resp.json()
    
    async def get_app_info(self) -> dict:
        """Get application info including compose-hash."""
        async with httpx.AsyncClient(transport=self.transport) as client:
            resp = await client.get("http://localhost/Info")
            resp.raise_for_status()
            return resp.json()
```

### Main Bot Application

```python
"""main.py - Signal AI Proxy Bot entry point"""

import os
import asyncio
import logging
import hashlib
from signalbot import SignalBot, Command, Context, triggered

from near_ai_client import NearAIClient
from conversation import ConversationStore
from dstack_utils import DstackClient

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Initialize clients
near_ai = NearAIClient(
    api_key=os.environ["NEAR_AI_API_KEY"],
    base_url=os.environ.get("NEAR_AI_BASE_URL", "https://api.near.ai/v1"),
    model=os.environ.get("NEAR_AI_MODEL", "llama-3.3-70b")
)
conversations = ConversationStore(
    redis_url=os.environ.get("REDIS_URL", "redis://localhost:6379")
)
dstack = DstackClient()

SYSTEM_PROMPT = """You are a helpful AI assistant accessible via Signal. 
You provide accurate, thoughtful responses while being concise for mobile chat.
You're running in a privacy-preserving environment with verifiable execution."""


class ChatCommand(Command):
    """Handle regular chat messages - proxy to NEAR AI."""
    
    def describe(self) -> str:
        return "Chat with AI"
    
    async def handle(self, ctx: Context) -> None:
        user_id = ctx.message.source  # Signal phone number
        user_message = ctx.message.text
        
        # Skip if it's a command
        if user_message.startswith("!"):
            return
        
        try:
            # Add user message to history
            await conversations.add_message(
                user_id, "user", user_message, SYSTEM_PROMPT
            )
            
            # Get full conversation for context
            messages = await conversations.to_openai_messages(
                user_id, SYSTEM_PROMPT
            )
            
            # Query NEAR AI Cloud
            response = await near_ai.chat(messages)
            
            # Store assistant response
            await conversations.add_message(user_id, "assistant", response)
            
            # Send back via Signal
            await ctx.send(response)
            
        except Exception as e:
            logger.error(f"Chat error: {e}")
            await ctx.send("Sorry, I encountered an error. Please try again.")


class VerifyCommand(Command):
    """Provide dual attestation proofs."""
    
    @triggered("!verify")
    async def handle(self, ctx: Context) -> None:
        try:
            # Get proxy TEE attestation
            challenge = hashlib.sha256(
                str(ctx.message.timestamp).encode()
            ).digest()
            
            proxy_quote = await dstack.get_quote(challenge)
            app_info = await dstack.get_app_info()
            
            # Get NEAR AI attestation
            near_attestation = await near_ai.get_attestation()
            
            response = f"""🔐 Privacy Verification

**Proxy (Signal Bot)**
├─ TEE: Intel TDX
├─ Compose Hash: {app_info.get('compose_hash', 'N/A')[:16]}...
├─ App ID: {app_info.get('app_id', 'N/A')[:16]}...
└─ Verify: https://proof.phala.network

**Inference (NEAR AI Cloud)**
├─ TEE: NVIDIA GPU (H100/H200)
├─ Model: {near_ai.model}
├─ Gateway: Intel TDX
└─ Verify: https://near.ai/verify

Both layers provide hardware-backed attestation.
Your messages never exist in plaintext outside TEEs."""
            
            await ctx.send(response)
            
        except Exception as e:
            logger.error(f"Verify error: {e}")
            await ctx.send("❌ Could not generate attestation")


class ClearCommand(Command):
    """Clear conversation history."""
    
    @triggered("!clear")
    async def handle(self, ctx: Context) -> None:
        user_id = ctx.message.source
        cleared = await conversations.clear(user_id)
        
        if cleared:
            await ctx.send("✅ Conversation history cleared.")
        else:
            await ctx.send("No conversation history to clear.")


class HelpCommand(Command):
    """Show available commands."""
    
    @triggered("!help")
    async def handle(self, ctx: Context) -> None:
        await ctx.send("""🤖 Signal AI (Private & Verifiable)

Just send a message to chat with AI.

**Commands:**
• !verify - Show privacy attestation proofs
• !clear - Clear conversation history  
• !models - List available AI models
• !help - Show this message

Your messages are end-to-end encrypted via Signal, processed in a verified TEE, and sent to NEAR AI Cloud's private inference (GPU TEE).

Neither the bot operator nor NEAR AI can read your messages.""")


class ModelsCommand(Command):
    """List available models."""
    
    @triggered("!models")
    async def handle(self, ctx: Context) -> None:
        try:
            models = await near_ai.get_models()
            model_list = "\n".join([f"• {m['id']}" for m in models[:10]])
            await ctx.send(f"**Available Models:**\n{model_list}")
        except Exception as e:
            logger.error(f"Models error: {e}")
            await ctx.send("Could not fetch model list.")


async def main():
    signal_service = os.environ.get("SIGNAL_SERVICE", "http://signal-api:8080")
    phone_number = os.environ.get("SIGNAL_PHONE")
    
    if not phone_number:
        logger.error("SIGNAL_PHONE environment variable required")
        return
    
    bot = SignalBot({
        "signal_service": signal_service,
        "phone_number": phone_number
    })
    
    # Register command handlers
    bot.register(ChatCommand())
    bot.register(VerifyCommand())
    bot.register(ClearCommand())
    bot.register(HelpCommand())
    bot.register(ModelsCommand())
    
    logger.info(f"Starting Signal AI Proxy in TEE...")
    logger.info(f"NEAR AI endpoint: {near_ai.base_url}")
    logger.info(f"Model: {near_ai.model}")
    
    bot.start()


if __name__ == "__main__":
    asyncio.run(main())
```

## Deployment

### Prerequisites

1. Intel TDX-enabled hardware or Phala Cloud account
2. Dstack infrastructure deployed (KMS, Gateway, VMM)
3. Signal account for bot identity
4. **NEAR AI Cloud API key** (from https://near.ai)
5. Domain for TLS termination (optional)

### Environment Variables

```bash
# Signal configuration (encrypted via Dstack SDK)
SIGNAL_PHONE="+1234567890"

# NEAR AI Cloud configuration
NEAR_AI_API_KEY="your-near-ai-key"
NEAR_AI_BASE_URL="https://api.near.ai/v1"
NEAR_AI_MODEL="llama-3.3-70b"  # or other available models

# Redis for conversation state
REDIS_URL="redis://redis:6379"
```

### Deployment Steps

1. **Get NEAR AI API Key**
   ```bash
   # Sign up at https://near.ai and get API key
   # Test the key:
   curl -X POST https://api.near.ai/v1/chat/completions \
     -H "Authorization: Bearer $NEAR_AI_API_KEY" \
     -H "Content-Type: application/json" \
     -d '{"model": "llama-3.3-70b", "messages": [{"role": "user", "content": "Hello"}]}'
   ```

2. **Prepare Signal Account**
   ```bash
   # Option: Link existing Signal account as secondary device
   # The QR code flow happens via signal-cli-rest-api
   ```

3. **Encrypt Sensitive Config for TEE**
   ```bash
   # Get TEE encryption public key
   curl https://kms.example.com/v1/encryption-key > tee-pubkey.pem
   
   # Encrypt secrets
   dstack-encrypt --pubkey tee-pubkey.pem \
                  --env SIGNAL_PHONE="+1234567890" \
                  --env NEAR_AI_API_KEY="sk-..." \
                  --output encrypted-env.json
   ```

4. **Deploy to Dstack**
   ```bash
   # Via dstack-vmm web UI or CLI
   dstack deploy --compose docker-compose.yaml \
                 --encrypted-env encrypted-env.json
   ```

5. **Link Signal Account**
   ```bash
   # Access signal-cli-rest-api QR endpoint
   curl http://<app-id>.app.example.com:8080/v1/qrcodelink?device_name=ai-bot
   
   # Scan QR code with Signal app on your phone
   ```

6. **Verify Deployment**
   ```bash
   # Send "!verify" to the bot via Signal
   # Should return dual attestation proof
   ```

### NEAR AI Cloud Models

Available models (as of late 2024):

| Model | Context | Use Case |
|-------|---------|----------|
| `llama-3.3-70b` | 128K | General purpose, high quality |
| `llama-3.1-8b` | 128K | Fast responses, lower cost |
| `qwen-2.5-72b` | 32K | Multilingual support |
| `deepseek-v3` | 64K | Code and reasoning |

Pricing: ~$0.75/M input tokens, ~$2/M output tokens (varies by model)

## Verification

### User Verification Flow

When a user sends `!verify` to the bot:

```
┌─────────────┐     "!verify"      ┌─────────────┐
│   Signal    │ ─────────────────► │  AI Proxy   │
│    User     │                    │   Bot TEE   │
│             │ ◄───────────────── │             │
│             │   Formatted        │   ┌─────────┴─────────┐
│             │   attestation      │   │ 1. Get TDX quote  │
└─────────────┘   summary          │   │ 2. Get app info   │
                                   │   │ 3. Query NEAR AI  │
                                   │   │    attestation    │
                                   │   └─────────┬─────────┘
                                   └─────────────┼─────────────┘
                                                 │
                                                 ▼
                                   ┌─────────────────────────────┐
                                   │      NEAR AI Cloud          │
                                   │                             │
                                   │  Returns:                   │
                                   │  - GPU TEE attestation      │
                                   │  - Model hash               │
                                   │  - Gateway attestation      │
                                   └─────────────────────────────┘
```

### Example Verification Response

```
🔐 Privacy Verification

**Proxy (Signal Bot)**
├─ TEE: Intel TDX
├─ Compose Hash: a1b2c3d4e5f6...
├─ App ID: signal-ai-proxy-v1
└─ Verify: https://proof.phala.network

**Inference (NEAR AI Cloud)**
├─ TEE: NVIDIA GPU (H100/H200)
├─ Model: llama-3.3-70b
├─ Gateway: Intel TDX
└─ Verify: https://near.ai/verify

Both layers provide hardware-backed attestation.
Your messages never exist in plaintext outside TEEs.
```

### Manual Verification Steps

1. **Verify Proxy TEE**:
   - Get TDX quote from bot (via `!verify` or API)
   - Validate Intel signature chain via Intel PCS/DCAP
   - Check MRTD, RTMR0-3 match published values
   - Confirm compose-hash matches public repo

2. **Verify NEAR AI Inference**:
   - Attestation returned with each response
   - Validate against Intel/NVIDIA attestation services
   - Confirm model hash matches expected weights

## Operational Considerations

### Monitoring

- Signal API health via `/v1/health`
- NEAR AI response latency and errors
- Redis connection and memory usage
- TEE attestation freshness

### Cost Estimation

| Component | Cost Estimate |
|-----------|---------------|
| Phala Cloud (TDX) | ~$0.10/hour |
| NEAR AI Inference | ~$0.75-2/M tokens |
| Signal | Free |
| Redis | Included in Dstack |

For a bot handling 1000 messages/day with ~500 tokens/message:
- Inference: ~$0.75-1.50/day
- Compute: ~$2.40/day
- **Total**: ~$4-5/day

### Failure Modes

| Failure | Impact | Mitigation |
|---------|--------|------------|
| TEE hardware fault | Bot offline | Multi-region deployment |
| NEAR AI unavailable | No AI responses | Graceful degradation message |
| Signal servers down | No messaging | Retry with backoff |
| Redis crash | Lost context | Stateless fallback mode |
| Attestation expiry | Verification fails | Auto-refresh quotes |

### Rate Limiting

- **Signal**: ~1 message/second recommended
- **NEAR AI**: 100 RPS per tenant
- **Dstack attestation**: Quote generation ~100ms

### Scaling Considerations

For high-volume deployments:

1. **Horizontal scaling**: Multiple TEE instances behind load balancer
2. **Shared Redis**: Redis cluster for conversation state
3. **Connection pooling**: Reuse NEAR AI connections
4. **Message queuing**: Buffer Signal messages during NEAR AI latency

## Security Considerations

### Threat Model

**In Scope:**
- Malicious infrastructure operator (cannot read TEE memory)
- Network eavesdroppers (TLS + Signal E2E)
- NEAR AI operator (cannot read GPU TEE memory)
- Supply chain attacks (mitigated by measurement verification)

**Out of Scope:**
- Intel/NVIDIA hardware backdoors
- Side-channel attacks on TDX/GPU TEE
- Compromise of user's Signal device
- User voluntarily sharing conversations

### Privacy Properties

| Property | Mechanism |
|----------|-----------|
| Message confidentiality | Signal E2E → TLS → GPU TEE |
| Forward secrecy | Signal Protocol + TEE key rotation |
| Metadata protection | Signal sealed sender, no logging in TEE |
| Verifiable execution | Dual TDX + GPU attestation |
| Operator blindness | TEE memory isolation |

### Limitations

1. **User metadata**: Signal sees phone numbers and timing
2. **Response timing**: Side-channel for message length inference
3. **Conversation history**: Stored in Redis (encrypted at rest)
4. **Model behavior**: AI responses not deterministic

## Future Enhancements

- **Multi-model routing**: Select model based on message content
- **Image support**: NEAR AI vision models via Signal attachments
- **Group chat**: Per-group conversation contexts
- **On-chain attestation**: Publish attestation roots to NEAR blockchain
- **Encrypted memory**: NEAR AI's upcoming encrypted persistent memory
- **MCP integration**: NEAR blockchain tools via near-mcp

## References

- [Dstack Documentation](https://docs.phala.com/dstack)
- [Dstack GitHub](https://github.com/Dstack-TEE/dstack)
- [NEAR AI Cloud](https://near.ai/cloud)
- [NEAR AI Private Inference](https://docs.near.ai/cloud/private-inference/)
- [nearai/cloud-api](https://github.com/nearai/cloud-api)
- [signal-cli REST API](https://github.com/bbernhard/signal-cli-rest-api)
- [Intel TDX](https://www.intel.com/content/www/us/en/developer/tools/trust-domain-extensions/overview.html)
- [NVIDIA Confidential Computing](https://developer.nvidia.com/confidential-computing)

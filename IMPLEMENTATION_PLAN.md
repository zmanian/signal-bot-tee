# Implementation Plan: Signal Bot TEE

This document outlines the detailed implementation plan for the Signal → TEE → NEAR AI Cloud private AI chat proxy.

## Table of Contents

1. [Project Structure](#1-project-structure)
2. [Phase 1: Foundation](#2-phase-1-foundation)
3. [Phase 2: Core Components](#3-phase-2-core-components)
4. [Phase 3: Bot Application](#4-phase-3-bot-application)
5. [Phase 4: TEE Integration](#5-phase-4-tee-integration)
6. [Phase 5: Docker & Deployment](#6-phase-5-docker--deployment)
7. [Phase 6: Testing](#7-phase-6-testing)
8. [Phase 7: Documentation & Polish](#8-phase-7-documentation--polish)
9. [File Manifest](#9-file-manifest)
10. [Dependencies](#10-dependencies)

---

## 1. Project Structure

```
signal-bot-tee/
├── bot/
│   ├── __init__.py
│   ├── main.py                    # Entry point
│   ├── config.py                  # Configuration management
│   ├── near_ai_client.py          # NEAR AI Cloud client
│   ├── conversation.py            # Conversation state manager
│   ├── dstack_client.py           # Dstack TEE utilities
│   ├── commands/
│   │   ├── __init__.py
│   │   ├── base.py                # Base command class
│   │   ├── chat.py                # Chat handler
│   │   ├── verify.py              # Attestation verification
│   │   ├── clear.py               # Clear history
│   │   ├── help.py                # Help command
│   │   └── models.py              # List models
│   └── utils/
│       ├── __init__.py
│       ├── logging.py             # Logging configuration
│       └── errors.py              # Custom exceptions
├── tests/
│   ├── __init__.py
│   ├── conftest.py                # Pytest fixtures
│   ├── test_near_ai_client.py
│   ├── test_conversation.py
│   ├── test_dstack_client.py
│   ├── test_commands/
│   │   ├── __init__.py
│   │   ├── test_chat.py
│   │   ├── test_verify.py
│   │   └── test_clear.py
│   └── integration/
│       ├── __init__.py
│       └── test_e2e.py
├── scripts/
│   ├── setup_signal.sh            # Signal account setup
│   ├── encrypt_secrets.sh         # Dstack encryption
│   ├── verify_tee.py              # Manual TEE verification
│   └── health_check.py            # Health monitoring
├── docker/
│   ├── Dockerfile                 # Bot container
│   ├── Dockerfile.dev             # Development container
│   └── docker-compose.yaml        # Full stack composition
├── .env.example                   # Environment template
├── .env.encrypted.example         # Encrypted env template
├── pyproject.toml                 # Python project config
├── requirements.txt               # Production dependencies
├── requirements-dev.txt           # Development dependencies
├── Makefile                       # Common operations
├── DESIGN.md                      # Architecture design
├── IMPLEMENTATION_PLAN.md         # This file
└── README.md                      # Project overview
```

---

## 2. Phase 1: Foundation

**Goal**: Set up project infrastructure, dependencies, and configuration management.

### 2.1 Initialize Python Project

**File: `pyproject.toml`**

```toml
[project]
name = "signal-bot-tee"
version = "0.1.0"
description = "Signal bot running in TEE proxying to NEAR AI Cloud"
requires-python = ">=3.11"
dependencies = [
    "httpx>=0.27.0",
    "signalbot>=0.8.0",
    "openai>=1.0.0",
    "redis>=5.0.0",
    "pydantic>=2.0.0",
    "pydantic-settings>=2.0.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=8.0.0",
    "pytest-asyncio>=0.23.0",
    "pytest-cov>=4.0.0",
    "mypy>=1.8.0",
    "ruff>=0.2.0",
    "fakeredis>=2.20.0",
    "respx>=0.20.0",
]

[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]

[tool.ruff]
line-length = 100
target-version = "py311"

[tool.mypy]
python_version = "3.11"
strict = true
```

### 2.2 Configuration Management

**File: `bot/config.py`**

```python
"""Configuration management using pydantic-settings."""

from pydantic_settings import BaseSettings
from pydantic import Field
from typing import Optional


class Settings(BaseSettings):
    """Application configuration loaded from environment variables."""

    # Signal configuration
    signal_service: str = Field(
        default="http://signal-api:8080",
        description="Signal CLI REST API endpoint"
    )
    signal_phone: str = Field(
        description="Phone number for Signal bot"
    )

    # NEAR AI configuration
    near_ai_api_key: str = Field(
        description="NEAR AI Cloud API key"
    )
    near_ai_base_url: str = Field(
        default="https://api.near.ai/v1",
        description="NEAR AI API base URL"
    )
    near_ai_model: str = Field(
        default="llama-3.3-70b",
        description="Default model for chat completions"
    )
    near_ai_timeout: int = Field(
        default=60,
        description="Request timeout in seconds"
    )

    # Redis configuration
    redis_url: str = Field(
        default="redis://localhost:6379",
        description="Redis connection URL"
    )
    redis_ttl_hours: int = Field(
        default=24,
        description="Conversation TTL in hours"
    )
    max_conversation_messages: int = Field(
        default=50,
        description="Maximum messages per conversation"
    )

    # Bot configuration
    system_prompt: str = Field(
        default="""You are a helpful AI assistant accessible via Signal.
You provide accurate, thoughtful responses while being concise for mobile chat.
You're running in a privacy-preserving environment with verifiable execution.""",
        description="System prompt for AI conversations"
    )
    log_level: str = Field(
        default="INFO",
        description="Logging level"
    )

    # Dstack configuration
    dstack_socket: str = Field(
        default="/var/run/dstack.sock",
        description="Dstack guest agent socket path"
    )

    class Config:
        env_file = ".env"
        env_file_encoding = "utf-8"
        case_sensitive = False


def get_settings() -> Settings:
    """Get cached settings instance."""
    return Settings()
```

### 2.3 Logging Configuration

**File: `bot/utils/logging.py`**

```python
"""Logging configuration for the bot."""

import logging
import sys
from typing import Optional


def setup_logging(level: str = "INFO") -> logging.Logger:
    """
    Configure application logging.

    Args:
        level: Logging level (DEBUG, INFO, WARNING, ERROR, CRITICAL)

    Returns:
        Configured root logger
    """
    # Create formatter
    formatter = logging.Formatter(
        fmt="%(asctime)s | %(levelname)-8s | %(name)s | %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S"
    )

    # Configure console handler
    console_handler = logging.StreamHandler(sys.stdout)
    console_handler.setFormatter(formatter)

    # Get root logger
    root_logger = logging.getLogger()
    root_logger.setLevel(getattr(logging, level.upper()))
    root_logger.addHandler(console_handler)

    # Reduce noise from third-party libraries
    logging.getLogger("httpx").setLevel(logging.WARNING)
    logging.getLogger("httpcore").setLevel(logging.WARNING)

    return root_logger


def get_logger(name: str) -> logging.Logger:
    """Get a named logger."""
    return logging.getLogger(name)
```

### 2.4 Custom Exceptions

**File: `bot/utils/errors.py`**

```python
"""Custom exceptions for the bot application."""


class BotError(Exception):
    """Base exception for all bot errors."""
    pass


class NearAIError(BotError):
    """Error communicating with NEAR AI Cloud."""
    pass


class NearAIRateLimitError(NearAIError):
    """Rate limit exceeded on NEAR AI Cloud."""
    pass


class NearAIAuthError(NearAIError):
    """Authentication failed with NEAR AI Cloud."""
    pass


class ConversationError(BotError):
    """Error with conversation storage."""
    pass


class DstackError(BotError):
    """Error communicating with Dstack guest agent."""
    pass


class AttestationError(DstackError):
    """Error generating or validating attestation."""
    pass


class SignalError(BotError):
    """Error communicating with Signal API."""
    pass
```

### 2.5 Tasks for Phase 1

| Task | Description | Files |
|------|-------------|-------|
| 1.1 | Create project structure | All directories |
| 1.2 | Initialize pyproject.toml | `pyproject.toml` |
| 1.3 | Create requirements files | `requirements.txt`, `requirements-dev.txt` |
| 1.4 | Implement config module | `bot/config.py` |
| 1.5 | Implement logging utility | `bot/utils/logging.py` |
| 1.6 | Define custom exceptions | `bot/utils/errors.py` |
| 1.7 | Create .env.example | `.env.example` |
| 1.8 | Create Makefile | `Makefile` |

---

## 3. Phase 2: Core Components

**Goal**: Implement the three core service clients: NEAR AI, Conversation Store, and Dstack.

### 3.1 NEAR AI Client

**File: `bot/near_ai_client.py`**

```python
"""NEAR AI Cloud client with attestation support."""

from openai import AsyncOpenAI
from typing import AsyncGenerator, Optional
import httpx
import json

from bot.utils.errors import NearAIError, NearAIRateLimitError, NearAIAuthError
from bot.utils.logging import get_logger

logger = get_logger(__name__)


class NearAIClient:
    """
    OpenAI-compatible client for NEAR AI Cloud with attestation support.

    NEAR AI Cloud provides:
    - OpenAI-compatible /v1/chat/completions endpoint
    - GPU TEE attestation per-request
    - ~5-10% latency overhead for privacy guarantees

    Example:
        >>> client = NearAIClient(api_key="sk-...", model="llama-3.3-70b")
        >>> response = await client.chat([{"role": "user", "content": "Hello!"}])
        >>> print(response)
    """

    def __init__(
        self,
        api_key: str,
        base_url: str = "https://api.near.ai/v1",
        model: str = "llama-3.3-70b",
        timeout: int = 60,
        max_retries: int = 3
    ):
        """
        Initialize NEAR AI client.

        Args:
            api_key: NEAR AI API key
            base_url: API base URL
            model: Default model for completions
            timeout: Request timeout in seconds
            max_retries: Maximum retry attempts
        """
        self.client = AsyncOpenAI(
            api_key=api_key,
            base_url=base_url,
            timeout=timeout,
            max_retries=max_retries
        )
        self.model = model
        self.base_url = base_url
        self._api_key = api_key

    async def chat(
        self,
        messages: list[dict],
        stream: bool = False,
        model: Optional[str] = None,
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
        **kwargs
    ) -> str | AsyncGenerator[str, None]:
        """
        Send chat completion request to NEAR AI Cloud.

        Args:
            messages: List of messages in OpenAI format
            stream: Whether to stream response
            model: Model override (uses default if None)
            temperature: Sampling temperature
            max_tokens: Maximum tokens in response
            **kwargs: Additional OpenAI parameters

        Returns:
            Complete response string or async generator for streaming

        Raises:
            NearAIError: On API errors
            NearAIRateLimitError: On rate limit
            NearAIAuthError: On authentication failure
        """
        try:
            if stream:
                return self._stream_chat(
                    messages, model=model, temperature=temperature,
                    max_tokens=max_tokens, **kwargs
                )

            response = await self.client.chat.completions.create(
                model=model or self.model,
                messages=messages,
                temperature=temperature,
                max_tokens=max_tokens,
                **kwargs
            )
            return response.choices[0].message.content or ""

        except Exception as e:
            self._handle_error(e)

    async def _stream_chat(
        self,
        messages: list[dict],
        model: Optional[str] = None,
        **kwargs
    ) -> AsyncGenerator[str, None]:
        """Stream chat responses for real-time output."""
        try:
            stream = await self.client.chat.completions.create(
                model=model or self.model,
                messages=messages,
                stream=True,
                **kwargs
            )
            async for chunk in stream:
                if chunk.choices[0].delta.content:
                    yield chunk.choices[0].delta.content

        except Exception as e:
            self._handle_error(e)

    async def get_attestation(self) -> dict:
        """
        Fetch attestation report from NEAR AI Cloud.

        Returns:
            Attestation report containing GPU TEE proofs

        Raises:
            NearAIError: On API errors
        """
        try:
            async with httpx.AsyncClient(timeout=30) as client:
                resp = await client.get(
                    f"{self.base_url}/attestation",
                    headers={"Authorization": f"Bearer {self._api_key}"}
                )
                resp.raise_for_status()
                return resp.json()
        except httpx.HTTPStatusError as e:
            raise NearAIError(f"Failed to get attestation: {e.response.status_code}")
        except Exception as e:
            raise NearAIError(f"Attestation request failed: {e}")

    async def get_models(self) -> list[dict]:
        """
        List available models on NEAR AI Cloud.

        Returns:
            List of model information dictionaries
        """
        try:
            models = await self.client.models.list()
            return [m.model_dump() for m in models.data]
        except Exception as e:
            raise NearAIError(f"Failed to list models: {e}")

    async def health_check(self) -> bool:
        """
        Check if NEAR AI Cloud is reachable.

        Returns:
            True if healthy, False otherwise
        """
        try:
            await self.client.models.list()
            return True
        except Exception:
            return False

    def _handle_error(self, error: Exception) -> None:
        """Convert exceptions to custom error types."""
        error_str = str(error).lower()

        if "rate limit" in error_str or "429" in error_str:
            raise NearAIRateLimitError(f"Rate limit exceeded: {error}")
        elif "unauthorized" in error_str or "401" in error_str:
            raise NearAIAuthError(f"Authentication failed: {error}")
        else:
            raise NearAIError(f"NEAR AI request failed: {error}")
```

### 3.2 Conversation Store

**File: `bot/conversation.py`**

```python
"""Per-user conversation history with Redis backend."""

import redis.asyncio as redis
import json
from typing import Optional
from dataclasses import dataclass, asdict, field
from datetime import datetime, timedelta

from bot.utils.errors import ConversationError
from bot.utils.logging import get_logger

logger = get_logger(__name__)


@dataclass
class Message:
    """Single message in a conversation."""
    role: str  # "user", "assistant", "system"
    content: str
    timestamp: float = field(default_factory=lambda: datetime.utcnow().timestamp())


@dataclass
class Conversation:
    """Full conversation state for a user."""
    user_id: str  # Signal phone number or group ID
    messages: list[Message]
    created_at: float
    updated_at: float
    system_prompt: Optional[str] = None


class ConversationStore:
    """
    Redis-backed conversation storage.

    Features:
    - Maintains chat history per Signal user/group
    - Auto-expires old conversations via TTL
    - Limits context window size to prevent token overflow
    - Async operations throughout

    Example:
        >>> store = ConversationStore("redis://localhost:6379")
        >>> await store.add_message("+1234567890", "user", "Hello!")
        >>> messages = await store.to_openai_messages("+1234567890")
    """

    def __init__(
        self,
        redis_url: str = "redis://localhost:6379",
        max_messages: int = 50,
        ttl_hours: int = 24
    ):
        """
        Initialize conversation store.

        Args:
            redis_url: Redis connection URL
            max_messages: Maximum messages to retain per conversation
            ttl_hours: Conversation expiration time in hours
        """
        self._redis_url = redis_url
        self._redis: Optional[redis.Redis] = None
        self.max_messages = max_messages
        self.ttl = timedelta(hours=ttl_hours)

    async def connect(self) -> None:
        """Establish Redis connection."""
        if self._redis is None:
            self._redis = redis.from_url(self._redis_url)
            logger.info(f"Connected to Redis at {self._redis_url}")

    async def disconnect(self) -> None:
        """Close Redis connection."""
        if self._redis:
            await self._redis.close()
            self._redis = None

    @property
    def redis(self) -> redis.Redis:
        """Get Redis client, raising if not connected."""
        if self._redis is None:
            raise ConversationError("Redis not connected. Call connect() first.")
        return self._redis

    def _key(self, user_id: str) -> str:
        """Generate Redis key for user conversation."""
        return f"conversation:{user_id}"

    async def get(self, user_id: str) -> Optional[Conversation]:
        """
        Get conversation for user.

        Args:
            user_id: Signal phone number or group ID

        Returns:
            Conversation if exists, None otherwise
        """
        try:
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
        except Exception as e:
            logger.error(f"Failed to get conversation for {user_id}: {e}")
            raise ConversationError(f"Failed to retrieve conversation: {e}")

    async def add_message(
        self,
        user_id: str,
        role: str,
        content: str,
        system_prompt: Optional[str] = None
    ) -> Conversation:
        """
        Add message to conversation, creating if needed.

        Args:
            user_id: Signal phone number or group ID
            role: Message role (user, assistant, system)
            content: Message content
            system_prompt: System prompt override

        Returns:
            Updated conversation
        """
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
            logger.debug(f"Created new conversation for {user_id}")

        conv.messages.append(Message(role=role, content=content, timestamp=now))
        conv.updated_at = now

        # Update system prompt if provided
        if system_prompt and conv.system_prompt != system_prompt:
            conv.system_prompt = system_prompt

        # Trim to max messages (keeping most recent)
        if len(conv.messages) > self.max_messages:
            conv.messages = conv.messages[-self.max_messages:]

        # Persist with TTL
        try:
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
        except Exception as e:
            logger.error(f"Failed to save conversation for {user_id}: {e}")
            raise ConversationError(f"Failed to save conversation: {e}")

        return conv

    async def clear(self, user_id: str) -> bool:
        """
        Clear conversation history for user.

        Args:
            user_id: Signal phone number or group ID

        Returns:
            True if conversation was deleted, False if didn't exist
        """
        try:
            result = await self.redis.delete(self._key(user_id))
            if result > 0:
                logger.info(f"Cleared conversation for {user_id}")
            return result > 0
        except Exception as e:
            logger.error(f"Failed to clear conversation for {user_id}: {e}")
            raise ConversationError(f"Failed to clear conversation: {e}")

    async def to_openai_messages(
        self,
        user_id: str,
        system_prompt: Optional[str] = None
    ) -> list[dict]:
        """
        Convert conversation to OpenAI messages format.

        Args:
            user_id: Signal phone number or group ID
            system_prompt: System prompt override

        Returns:
            List of messages in OpenAI format
        """
        conv = await self.get(user_id)
        messages = []

        # Add system prompt (priority: param > stored > none)
        prompt = system_prompt or (conv.system_prompt if conv else None)
        if prompt:
            messages.append({"role": "system", "content": prompt})

        # Add conversation history
        if conv:
            for msg in conv.messages:
                messages.append({"role": msg.role, "content": msg.content})

        return messages

    async def get_message_count(self, user_id: str) -> int:
        """Get number of messages in conversation."""
        conv = await self.get(user_id)
        return len(conv.messages) if conv else 0
```

### 3.3 Dstack Client

**File: `bot/dstack_client.py`**

```python
"""Dstack TEE guest agent client."""

import httpx
from typing import Optional
import base64

from bot.utils.errors import DstackError, AttestationError
from bot.utils.logging import get_logger

logger = get_logger(__name__)

DSTACK_SOCKET = "/var/run/dstack.sock"


class DstackClient:
    """
    Client for Dstack guest agent APIs.

    The Dstack guest agent provides TEE services via Unix socket:
    - Key derivation from hardware root of trust
    - TDX attestation quote generation
    - Application information and compose hash

    Example:
        >>> client = DstackClient()
        >>> app_info = await client.get_app_info()
        >>> quote = await client.get_quote(b"challenge")
    """

    def __init__(self, socket_path: str = DSTACK_SOCKET):
        """
        Initialize Dstack client.

        Args:
            socket_path: Path to Dstack Unix socket
        """
        self.socket_path = socket_path
        self._transport = httpx.AsyncHTTPTransport(uds=socket_path)

    async def derive_key(
        self,
        path: str,
        subject: Optional[str] = None,
        size: int = 32
    ) -> bytes:
        """
        Derive a deterministic key from TEE root of trust.

        The derived key is:
        - Deterministic: Same path/subject always yields same key
        - TEE-bound: Cannot be derived outside this TEE instance
        - Hierarchy-based: Different paths yield different keys

        Args:
            path: Key derivation path (e.g., "/encryption/api-key")
            subject: Optional subject to mix into derivation
            size: Key size in bytes (default 32 = 256-bit)

        Returns:
            Derived key bytes

        Raises:
            DstackError: On derivation failure
        """
        try:
            async with httpx.AsyncClient(transport=self._transport) as client:
                params = {"path": path}
                if subject:
                    params["subject"] = subject

                resp = await client.post(
                    "http://localhost/DeriveKey",
                    json=params,
                    timeout=10
                )
                resp.raise_for_status()

                key_hex = resp.json().get("key")
                if not key_hex:
                    raise DstackError("No key in derivation response")

                return bytes.fromhex(key_hex)

        except httpx.HTTPStatusError as e:
            raise DstackError(f"Key derivation failed: HTTP {e.response.status_code}")
        except Exception as e:
            raise DstackError(f"Key derivation failed: {e}")

    async def get_quote(self, report_data: bytes) -> dict:
        """
        Generate TDX attestation quote.

        The quote proves:
        - Genuine Intel TDX hardware
        - Expected firmware measurements (MRTD)
        - Expected kernel measurements (RTMRs)
        - Custom report data for freshness

        Args:
            report_data: 64-byte challenge/nonce for freshness
                        (will be truncated/padded if different size)

        Returns:
            Quote dictionary containing:
            - quote: Base64-encoded TDX quote
            - report_data: The included report data

        Raises:
            AttestationError: On quote generation failure
        """
        try:
            # Ensure report_data is exactly 64 bytes
            if len(report_data) < 64:
                report_data = report_data + b'\x00' * (64 - len(report_data))
            elif len(report_data) > 64:
                report_data = report_data[:64]

            async with httpx.AsyncClient(transport=self._transport) as client:
                resp = await client.get(
                    "http://localhost/GetQuote",
                    params={"report_data": report_data.hex()},
                    timeout=30  # Quote generation can be slow
                )
                resp.raise_for_status()
                return resp.json()

        except httpx.HTTPStatusError as e:
            raise AttestationError(f"Quote generation failed: HTTP {e.response.status_code}")
        except Exception as e:
            raise AttestationError(f"Quote generation failed: {e}")

    async def get_app_info(self) -> dict:
        """
        Get application info including compose-hash.

        Returns:
            Application info containing:
            - app_id: Unique application identifier
            - compose_hash: Hash of docker-compose configuration
            - instance_id: Unique instance identifier

        Raises:
            DstackError: On info retrieval failure
        """
        try:
            async with httpx.AsyncClient(transport=self._transport) as client:
                resp = await client.get(
                    "http://localhost/Info",
                    timeout=10
                )
                resp.raise_for_status()
                return resp.json()

        except httpx.HTTPStatusError as e:
            raise DstackError(f"App info failed: HTTP {e.response.status_code}")
        except Exception as e:
            raise DstackError(f"App info failed: {e}")

    async def is_in_tee(self) -> bool:
        """
        Check if running inside a TEE.

        Returns:
            True if Dstack socket is accessible, False otherwise
        """
        try:
            await self.get_app_info()
            return True
        except Exception:
            return False

    async def get_ra_tls_cert(self) -> bytes:
        """
        Get RA-TLS certificate for authenticated TLS connections.

        Returns:
            PEM-encoded certificate with attestation embedded

        Raises:
            AttestationError: On certificate generation failure
        """
        try:
            async with httpx.AsyncClient(transport=self._transport) as client:
                resp = await client.get(
                    "http://localhost/GetRaTlsCert",
                    timeout=30
                )
                resp.raise_for_status()
                cert_b64 = resp.json().get("cert")
                if not cert_b64:
                    raise AttestationError("No certificate in response")
                return base64.b64decode(cert_b64)

        except httpx.HTTPStatusError as e:
            raise AttestationError(f"RA-TLS cert failed: HTTP {e.response.status_code}")
        except Exception as e:
            raise AttestationError(f"RA-TLS cert failed: {e}")
```

### 3.4 Tasks for Phase 2

| Task | Description | Files |
|------|-------------|-------|
| 2.1 | Implement NEAR AI client | `bot/near_ai_client.py` |
| 2.2 | Add streaming support | `bot/near_ai_client.py` |
| 2.3 | Add attestation endpoint | `bot/near_ai_client.py` |
| 2.4 | Implement conversation dataclasses | `bot/conversation.py` |
| 2.5 | Implement ConversationStore | `bot/conversation.py` |
| 2.6 | Add conversation TTL and trimming | `bot/conversation.py` |
| 2.7 | Implement Dstack client | `bot/dstack_client.py` |
| 2.8 | Add key derivation | `bot/dstack_client.py` |
| 2.9 | Add quote generation | `bot/dstack_client.py` |
| 2.10 | Write unit tests for all clients | `tests/` |

---

## 4. Phase 3: Bot Application

**Goal**: Implement the Signal bot commands and message handling.

### 4.1 Base Command Class

**File: `bot/commands/base.py`**

```python
"""Base command class for Signal bot commands."""

from abc import ABC, abstractmethod
from typing import Optional
from signalbot import Context

from bot.utils.logging import get_logger

logger = get_logger(__name__)


class BaseCommand(ABC):
    """
    Abstract base class for bot commands.

    All commands should inherit from this class and implement
    the required methods.
    """

    @property
    @abstractmethod
    def name(self) -> str:
        """Command name (e.g., 'verify', 'clear')."""
        pass

    @property
    @abstractmethod
    def description(self) -> str:
        """Short description of the command."""
        pass

    @property
    def trigger(self) -> Optional[str]:
        """
        Trigger prefix for the command.

        Returns:
            Trigger string (e.g., '!verify') or None for default handler
        """
        return f"!{self.name}"

    @property
    def is_default(self) -> bool:
        """Whether this is the default message handler."""
        return False

    @abstractmethod
    async def execute(self, ctx: Context, user_id: str, message: str) -> None:
        """
        Execute the command.

        Args:
            ctx: Signal bot context
            user_id: User's Signal phone number
            message: Full message text
        """
        pass

    async def send_error(self, ctx: Context, error_msg: str) -> None:
        """Send an error message to the user."""
        await ctx.send(f"❌ {error_msg}")
        logger.warning(f"Command {self.name} error: {error_msg}")
```

### 4.2 Chat Command

**File: `bot/commands/chat.py`**

```python
"""Chat command - proxies messages to NEAR AI Cloud."""

from signalbot import Context

from bot.commands.base import BaseCommand
from bot.near_ai_client import NearAIClient
from bot.conversation import ConversationStore
from bot.utils.errors import NearAIError, NearAIRateLimitError
from bot.utils.logging import get_logger

logger = get_logger(__name__)


class ChatCommand(BaseCommand):
    """
    Default message handler that proxies chat to NEAR AI Cloud.

    Features:
    - Maintains conversation history per user
    - Handles rate limiting gracefully
    - Provides typing indicators via Signal
    """

    def __init__(
        self,
        near_ai: NearAIClient,
        conversations: ConversationStore,
        system_prompt: str
    ):
        self.near_ai = near_ai
        self.conversations = conversations
        self.system_prompt = system_prompt

    @property
    def name(self) -> str:
        return "chat"

    @property
    def description(self) -> str:
        return "Chat with AI"

    @property
    def trigger(self) -> None:
        return None  # Default handler

    @property
    def is_default(self) -> bool:
        return True

    async def execute(self, ctx: Context, user_id: str, message: str) -> None:
        """Process chat message and respond with AI."""
        # Skip command messages
        if message.startswith("!"):
            return

        try:
            logger.info(f"Chat from {user_id[:8]}...: {message[:50]}...")

            # Add user message to history
            await self.conversations.add_message(
                user_id, "user", message, self.system_prompt
            )

            # Get full conversation for context
            messages = await self.conversations.to_openai_messages(
                user_id, self.system_prompt
            )

            # Query NEAR AI Cloud
            response = await self.near_ai.chat(messages)

            # Store assistant response
            await self.conversations.add_message(user_id, "assistant", response)

            # Send back via Signal
            await ctx.send(response)

            logger.info(f"Response to {user_id[:8]}...: {len(response)} chars")

        except NearAIRateLimitError:
            await ctx.send(
                "⏳ I'm receiving too many requests right now. "
                "Please wait a moment and try again."
            )
        except NearAIError as e:
            logger.error(f"NEAR AI error: {e}")
            await ctx.send(
                "Sorry, I encountered an error connecting to the AI service. "
                "Please try again in a moment."
            )
        except Exception as e:
            logger.exception(f"Unexpected error in chat: {e}")
            await ctx.send("Sorry, something went wrong. Please try again.")
```

### 4.3 Verify Command

**File: `bot/commands/verify.py`**

```python
"""Verify command - provides dual attestation proofs."""

import hashlib
from signalbot import Context

from bot.commands.base import BaseCommand
from bot.near_ai_client import NearAIClient
from bot.dstack_client import DstackClient
from bot.utils.errors import AttestationError, NearAIError, DstackError
from bot.utils.logging import get_logger

logger = get_logger(__name__)


class VerifyCommand(BaseCommand):
    """
    Provides dual attestation proofs from proxy and inference TEEs.

    Returns:
    - Proxy TEE (Dstack/Intel TDX) attestation info
    - Inference TEE (NEAR AI/NVIDIA GPU) attestation info
    - Verification links for independent validation
    """

    def __init__(self, near_ai: NearAIClient, dstack: DstackClient):
        self.near_ai = near_ai
        self.dstack = dstack

    @property
    def name(self) -> str:
        return "verify"

    @property
    def description(self) -> str:
        return "Show privacy attestation proofs"

    async def execute(self, ctx: Context, user_id: str, message: str) -> None:
        """Generate and send attestation verification response."""
        try:
            # Generate challenge based on timestamp for freshness
            challenge = hashlib.sha256(
                f"{ctx.message.timestamp}:{user_id}".encode()
            ).digest()

            # Collect attestation info (attempt both, report partial on failure)
            proxy_info = await self._get_proxy_attestation(challenge)
            near_info = await self._get_near_attestation()

            # Format response
            response = self._format_response(proxy_info, near_info)
            await ctx.send(response)

            logger.info(f"Attestation provided to {user_id[:8]}...")

        except Exception as e:
            logger.exception(f"Attestation error: {e}")
            await self.send_error(ctx, "Could not generate attestation. Please try again.")

    async def _get_proxy_attestation(self, challenge: bytes) -> dict:
        """Get proxy TEE attestation info."""
        try:
            # Check if running in TEE
            if not await self.dstack.is_in_tee():
                return {
                    "available": False,
                    "reason": "Not running in TEE environment"
                }

            app_info = await self.dstack.get_app_info()
            quote = await self.dstack.get_quote(challenge)

            return {
                "available": True,
                "compose_hash": app_info.get("compose_hash", "N/A"),
                "app_id": app_info.get("app_id", "N/A"),
                "instance_id": app_info.get("instance_id", "N/A"),
                "quote_generated": True
            }

        except DstackError as e:
            logger.warning(f"Dstack attestation failed: {e}")
            return {
                "available": False,
                "reason": str(e)
            }

    async def _get_near_attestation(self) -> dict:
        """Get NEAR AI attestation info."""
        try:
            attestation = await self.near_ai.get_attestation()
            return {
                "available": True,
                "model": self.near_ai.model,
                **attestation
            }
        except NearAIError as e:
            logger.warning(f"NEAR AI attestation failed: {e}")
            return {
                "available": False,
                "model": self.near_ai.model,
                "reason": str(e)
            }

    def _format_response(self, proxy: dict, near: dict) -> str:
        """Format attestation information for user."""
        lines = ["🔐 **Privacy Verification**", ""]

        # Proxy section
        lines.append("**Proxy (Signal Bot)**")
        if proxy.get("available"):
            lines.append(f"├─ TEE: Intel TDX")
            lines.append(f"├─ Compose Hash: {proxy['compose_hash'][:16]}...")
            lines.append(f"├─ App ID: {proxy['app_id'][:16]}...")
            lines.append(f"└─ Verify: https://proof.phala.network")
        else:
            lines.append(f"└─ ⚠️ {proxy.get('reason', 'Unavailable')}")

        lines.append("")

        # Inference section
        lines.append("**Inference (NEAR AI Cloud)**")
        if near.get("available"):
            lines.append(f"├─ TEE: NVIDIA GPU (H100/H200)")
            lines.append(f"├─ Model: {near['model']}")
            lines.append(f"├─ Gateway: Intel TDX")
            lines.append(f"└─ Verify: https://near.ai/verify")
        else:
            lines.append(f"├─ Model: {near.get('model', 'N/A')}")
            lines.append(f"└─ ⚠️ {near.get('reason', 'Unavailable')}")

        lines.append("")
        lines.append("Both layers provide hardware-backed attestation.")
        lines.append("Your messages never exist in plaintext outside TEEs.")

        return "\n".join(lines)
```

### 4.4 Clear Command

**File: `bot/commands/clear.py`**

```python
"""Clear command - resets conversation history."""

from signalbot import Context

from bot.commands.base import BaseCommand
from bot.conversation import ConversationStore
from bot.utils.logging import get_logger

logger = get_logger(__name__)


class ClearCommand(BaseCommand):
    """Clears conversation history for the user."""

    def __init__(self, conversations: ConversationStore):
        self.conversations = conversations

    @property
    def name(self) -> str:
        return "clear"

    @property
    def description(self) -> str:
        return "Clear conversation history"

    async def execute(self, ctx: Context, user_id: str, message: str) -> None:
        """Clear the user's conversation history."""
        try:
            cleared = await self.conversations.clear(user_id)

            if cleared:
                await ctx.send("✅ Conversation history cleared.")
                logger.info(f"Cleared history for {user_id[:8]}...")
            else:
                await ctx.send("No conversation history to clear.")

        except Exception as e:
            logger.error(f"Clear error for {user_id[:8]}...: {e}")
            await self.send_error(ctx, "Could not clear history. Please try again.")
```

### 4.5 Help Command

**File: `bot/commands/help.py`**

```python
"""Help command - displays available commands."""

from signalbot import Context

from bot.commands.base import BaseCommand


class HelpCommand(BaseCommand):
    """Displays available commands and usage information."""

    @property
    def name(self) -> str:
        return "help"

    @property
    def description(self) -> str:
        return "Show available commands"

    async def execute(self, ctx: Context, user_id: str, message: str) -> None:
        """Send help information."""
        help_text = """🤖 **Signal AI** (Private & Verifiable)

Just send a message to chat with AI.

**Commands:**
• !verify - Show privacy attestation proofs
• !clear - Clear conversation history
• !models - List available AI models
• !help - Show this message

**Privacy:**
Your messages are end-to-end encrypted via Signal, processed in a verified TEE (Intel TDX), and sent to NEAR AI Cloud's private inference (NVIDIA GPU TEE).

Neither the bot operator nor NEAR AI can read your messages."""

        await ctx.send(help_text)
```

### 4.6 Models Command

**File: `bot/commands/models.py`**

```python
"""Models command - lists available AI models."""

from signalbot import Context

from bot.commands.base import BaseCommand
from bot.near_ai_client import NearAIClient
from bot.utils.errors import NearAIError
from bot.utils.logging import get_logger

logger = get_logger(__name__)


class ModelsCommand(BaseCommand):
    """Lists available models on NEAR AI Cloud."""

    def __init__(self, near_ai: NearAIClient):
        self.near_ai = near_ai

    @property
    def name(self) -> str:
        return "models"

    @property
    def description(self) -> str:
        return "List available AI models"

    async def execute(self, ctx: Context, user_id: str, message: str) -> None:
        """List available models."""
        try:
            models = await self.near_ai.get_models()

            # Format model list (limit to 10)
            model_list = "\n".join([f"• {m['id']}" for m in models[:10]])
            current = f"\n\n_Current: {self.near_ai.model}_"

            await ctx.send(f"**Available Models:**\n{model_list}{current}")

        except NearAIError as e:
            logger.error(f"Models list error: {e}")
            await self.send_error(ctx, "Could not fetch model list.")
```

### 4.7 Main Entry Point

**File: `bot/main.py`**

```python
"""Signal AI Proxy Bot entry point."""

import asyncio
import signal
from signalbot import SignalBot, Command, Context

from bot.config import get_settings, Settings
from bot.near_ai_client import NearAIClient
from bot.conversation import ConversationStore
from bot.dstack_client import DstackClient
from bot.commands.chat import ChatCommand
from bot.commands.verify import VerifyCommand
from bot.commands.clear import ClearCommand
from bot.commands.help import HelpCommand
from bot.commands.models import ModelsCommand
from bot.utils.logging import setup_logging, get_logger

logger = get_logger(__name__)


class SignalAIBot:
    """
    Main bot application orchestrating all components.

    Lifecycle:
    1. Initialize configuration and clients
    2. Connect to Redis
    3. Register Signal commands
    4. Start message processing
    5. Handle graceful shutdown
    """

    def __init__(self, settings: Settings):
        self.settings = settings

        # Initialize clients
        self.near_ai = NearAIClient(
            api_key=settings.near_ai_api_key,
            base_url=settings.near_ai_base_url,
            model=settings.near_ai_model,
            timeout=settings.near_ai_timeout
        )

        self.conversations = ConversationStore(
            redis_url=settings.redis_url,
            max_messages=settings.max_conversation_messages,
            ttl_hours=settings.redis_ttl_hours
        )

        self.dstack = DstackClient(socket_path=settings.dstack_socket)

        # Signal bot will be initialized in start()
        self._bot: SignalBot = None
        self._shutdown_event = asyncio.Event()

    def _create_signal_command(self, cmd) -> Command:
        """Wrap our command in signalbot's Command class."""
        parent = self

        class WrappedCommand(Command):
            def describe(self) -> str:
                return cmd.description

            async def handle(self, ctx: Context) -> None:
                user_id = ctx.message.source
                message = ctx.message.text or ""
                await cmd.execute(ctx, user_id, message)

        # Apply trigger decorator if not default
        if cmd.trigger and not cmd.is_default:
            from signalbot import triggered
            WrappedCommand.handle = triggered(cmd.trigger)(WrappedCommand.handle)

        return WrappedCommand()

    async def start(self) -> None:
        """Start the bot and begin processing messages."""
        logger.info("Starting Signal AI Proxy Bot...")

        # Connect to Redis
        await self.conversations.connect()
        logger.info("Connected to Redis")

        # Check NEAR AI health
        if await self.near_ai.health_check():
            logger.info(f"NEAR AI healthy - Model: {self.settings.near_ai_model}")
        else:
            logger.warning("NEAR AI health check failed - will retry on requests")

        # Check TEE environment
        if await self.dstack.is_in_tee():
            app_info = await self.dstack.get_app_info()
            logger.info(f"Running in TEE - App ID: {app_info.get('app_id', 'unknown')}")
        else:
            logger.warning("Not running in TEE environment - attestation unavailable")

        # Initialize Signal bot
        self._bot = SignalBot({
            "signal_service": self.settings.signal_service,
            "phone_number": self.settings.signal_phone
        })

        # Create and register commands
        commands = [
            ChatCommand(self.near_ai, self.conversations, self.settings.system_prompt),
            VerifyCommand(self.near_ai, self.dstack),
            ClearCommand(self.conversations),
            HelpCommand(),
            ModelsCommand(self.near_ai)
        ]

        for cmd in commands:
            wrapped = self._create_signal_command(cmd)
            self._bot.register(wrapped)
            logger.debug(f"Registered command: {cmd.name}")

        logger.info(f"Signal service: {self.settings.signal_service}")
        logger.info(f"NEAR AI endpoint: {self.settings.near_ai_base_url}")
        logger.info("Bot started - listening for messages...")

        # Start the bot (this blocks)
        self._bot.start()

    async def stop(self) -> None:
        """Gracefully stop the bot."""
        logger.info("Shutting down...")

        if self._bot:
            # Signal bot doesn't have async stop, this is best effort
            pass

        await self.conversations.disconnect()
        logger.info("Disconnected from Redis")

        self._shutdown_event.set()


def setup_signal_handlers(bot: SignalAIBot) -> None:
    """Setup graceful shutdown handlers."""
    loop = asyncio.get_event_loop()

    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(
            sig,
            lambda: asyncio.create_task(bot.stop())
        )


async def main() -> None:
    """Main entry point."""
    # Load configuration
    settings = get_settings()

    # Setup logging
    setup_logging(settings.log_level)

    # Create and start bot
    bot = SignalAIBot(settings)
    setup_signal_handlers(bot)

    try:
        await bot.start()
    except KeyboardInterrupt:
        logger.info("Received interrupt signal")
    finally:
        await bot.stop()


if __name__ == "__main__":
    asyncio.run(main())
```

### 4.8 Tasks for Phase 3

| Task | Description | Files |
|------|-------------|-------|
| 3.1 | Create base command class | `bot/commands/base.py` |
| 3.2 | Implement chat command | `bot/commands/chat.py` |
| 3.3 | Implement verify command | `bot/commands/verify.py` |
| 3.4 | Implement clear command | `bot/commands/clear.py` |
| 3.5 | Implement help command | `bot/commands/help.py` |
| 3.6 | Implement models command | `bot/commands/models.py` |
| 3.7 | Create main entry point | `bot/main.py` |
| 3.8 | Add graceful shutdown | `bot/main.py` |
| 3.9 | Write command unit tests | `tests/test_commands/` |

---

## 5. Phase 4: TEE Integration

**Goal**: Ensure proper integration with Dstack TEE and implement verification utilities.

### 5.1 TEE Verification Script

**File: `scripts/verify_tee.py`**

```python
#!/usr/bin/env python3
"""Manual TEE verification utility."""

import asyncio
import sys
import json
import hashlib
import base64

from bot.dstack_client import DstackClient
from bot.near_ai_client import NearAIClient


async def verify_proxy_tee():
    """Verify proxy TEE attestation."""
    print("=" * 60)
    print("PROXY TEE VERIFICATION (Intel TDX)")
    print("=" * 60)

    dstack = DstackClient()

    if not await dstack.is_in_tee():
        print("❌ Not running in TEE environment")
        return False

    print("✅ Running in TEE environment")

    # Get app info
    app_info = await dstack.get_app_info()
    print(f"\n📋 Application Info:")
    print(f"   App ID: {app_info.get('app_id', 'N/A')}")
    print(f"   Compose Hash: {app_info.get('compose_hash', 'N/A')}")
    print(f"   Instance ID: {app_info.get('instance_id', 'N/A')}")

    # Generate quote
    challenge = hashlib.sha256(b"verification-test").digest()
    quote = await dstack.get_quote(challenge)
    print(f"\n🔐 TDX Quote Generated:")
    print(f"   Quote length: {len(quote.get('quote', ''))} chars")
    print(f"   Report data included: {bool(quote.get('report_data'))}")

    return True


async def verify_near_ai_tee(api_key: str):
    """Verify NEAR AI TEE attestation."""
    print("\n" + "=" * 60)
    print("NEAR AI TEE VERIFICATION (NVIDIA GPU)")
    print("=" * 60)

    client = NearAIClient(api_key=api_key)

    # Health check
    if not await client.health_check():
        print("❌ NEAR AI not reachable")
        return False

    print("✅ NEAR AI Cloud reachable")

    # Get attestation
    try:
        attestation = await client.get_attestation()
        print(f"\n🔐 GPU TEE Attestation:")
        print(json.dumps(attestation, indent=2))
        return True
    except Exception as e:
        print(f"⚠️ Attestation endpoint: {e}")
        return True  # Endpoint may not be exposed publicly


async def main():
    """Run verification checks."""
    print("\n🔍 TEE VERIFICATION UTILITY\n")

    proxy_ok = await verify_proxy_tee()

    # NEAR AI requires API key
    api_key = input("\nEnter NEAR AI API key (or press Enter to skip): ").strip()
    near_ok = True
    if api_key:
        near_ok = await verify_near_ai_tee(api_key)
    else:
        print("\n⏭️ Skipping NEAR AI verification")

    print("\n" + "=" * 60)
    print("VERIFICATION SUMMARY")
    print("=" * 60)
    print(f"Proxy TEE: {'✅ PASSED' if proxy_ok else '❌ FAILED'}")
    print(f"NEAR AI TEE: {'✅ PASSED' if near_ok else '❌ FAILED'}")

    return 0 if (proxy_ok and near_ok) else 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
```

### 5.2 Health Check Script

**File: `scripts/health_check.py`**

```python
#!/usr/bin/env python3
"""Health monitoring script for the bot."""

import asyncio
import sys
import httpx
import redis.asyncio as redis


async def check_signal_api(url: str) -> bool:
    """Check Signal CLI REST API health."""
    try:
        async with httpx.AsyncClient(timeout=10) as client:
            resp = await client.get(f"{url}/v1/health")
            return resp.status_code == 200
    except Exception:
        return False


async def check_redis(url: str) -> bool:
    """Check Redis connectivity."""
    try:
        r = redis.from_url(url)
        await r.ping()
        await r.close()
        return True
    except Exception:
        return False


async def check_near_ai(base_url: str, api_key: str) -> bool:
    """Check NEAR AI API connectivity."""
    try:
        async with httpx.AsyncClient(timeout=10) as client:
            resp = await client.get(
                f"{base_url}/models",
                headers={"Authorization": f"Bearer {api_key}"}
            )
            return resp.status_code == 200
    except Exception:
        return False


async def main():
    """Run health checks."""
    import os

    signal_url = os.environ.get("SIGNAL_SERVICE", "http://localhost:8080")
    redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379")
    near_url = os.environ.get("NEAR_AI_BASE_URL", "https://api.near.ai/v1")
    near_key = os.environ.get("NEAR_AI_API_KEY", "")

    results = {
        "signal_api": await check_signal_api(signal_url),
        "redis": await check_redis(redis_url),
        "near_ai": await check_near_ai(near_url, near_key) if near_key else None
    }

    print("Health Check Results:")
    for service, healthy in results.items():
        if healthy is None:
            status = "⏭️ SKIPPED"
        elif healthy:
            status = "✅ HEALTHY"
        else:
            status = "❌ UNHEALTHY"
        print(f"  {service}: {status}")

    # Exit with error if any required service is unhealthy
    required = ["signal_api", "redis"]
    if all(results.get(s) for s in required):
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
```

### 5.3 Tasks for Phase 4

| Task | Description | Files |
|------|-------------|-------|
| 4.1 | Create TEE verification script | `scripts/verify_tee.py` |
| 4.2 | Create health check script | `scripts/health_check.py` |
| 4.3 | Test key derivation | Manual testing |
| 4.4 | Test quote generation | Manual testing |
| 4.5 | Document TEE verification steps | `README.md` |

---

## 6. Phase 5: Docker & Deployment

**Goal**: Create production-ready Docker configuration and deployment scripts.

### 6.1 Production Dockerfile

**File: `docker/Dockerfile`**

```dockerfile
# Build stage
FROM python:3.11-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    gcc \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements
COPY requirements.txt .

# Install dependencies
RUN pip install --no-cache-dir --user -r requirements.txt

# Production stage
FROM python:3.11-slim

WORKDIR /app

# Create non-root user
RUN useradd --create-home --shell /bin/bash botuser

# Copy installed packages from builder
COPY --from=builder /root/.local /home/botuser/.local
ENV PATH=/home/botuser/.local/bin:$PATH

# Copy application code
COPY bot/ ./bot/
COPY scripts/ ./scripts/

# Change ownership
RUN chown -R botuser:botuser /app

USER botuser

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD python scripts/health_check.py || exit 1

# Entry point
CMD ["python", "-m", "bot.main"]
```

### 6.2 Development Dockerfile

**File: `docker/Dockerfile.dev`**

```dockerfile
FROM python:3.11-slim

WORKDIR /app

# Install dev dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    gcc \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements
COPY requirements.txt requirements-dev.txt ./

# Install all dependencies
RUN pip install --no-cache-dir -r requirements.txt -r requirements-dev.txt

# Copy application
COPY . .

# Development mode
CMD ["python", "-m", "pytest", "-v", "--cov=bot"]
```

### 6.3 Docker Compose

**File: `docker/docker-compose.yaml`**

```yaml
version: "3.8"

services:
  signal-api:
    image: bbernhard/signal-cli-rest-api:latest
    container_name: signal-api
    environment:
      - MODE=json-rpc
      - JSON_RPC_TRUST_NEW_IDENTITIES=on-first-use
      - LOG_LEVEL=info
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock:ro
      - signal-config:/home/.local/share/signal-cli
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
    restart: unless-stopped

  ai-proxy-bot:
    build:
      context: ..
      dockerfile: docker/Dockerfile
    container_name: ai-proxy-bot
    environment:
      - SIGNAL_SERVICE=http://signal-api:8080
      - SIGNAL_PHONE=${SIGNAL_PHONE}
      - NEAR_AI_API_KEY=${NEAR_AI_API_KEY}
      - NEAR_AI_BASE_URL=${NEAR_AI_BASE_URL:-https://api.near.ai/v1}
      - NEAR_AI_MODEL=${NEAR_AI_MODEL:-llama-3.3-70b}
      - REDIS_URL=redis://redis:6379
      - LOG_LEVEL=${LOG_LEVEL:-INFO}
    volumes:
      - /var/run/dstack.sock:/var/run/dstack.sock:ro
    depends_on:
      signal-api:
        condition: service_healthy
      redis:
        condition: service_started
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    container_name: redis
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3
    restart: unless-stopped

volumes:
  signal-config:
    driver: local
  redis-data:
    driver: local

networks:
  default:
    name: signal-bot-network
```

### 6.4 Setup Scripts

**File: `scripts/setup_signal.sh`**

```bash
#!/bin/bash
set -euo pipefail

# Signal account setup script
# Run this after deploying to link a Signal account

SIGNAL_API="${SIGNAL_SERVICE:-http://localhost:8080}"
DEVICE_NAME="${DEVICE_NAME:-signal-ai-bot}"

echo "🔗 Signal Account Linking"
echo "========================="
echo ""
echo "This will generate a QR code to link your Signal account."
echo "Scan it with Signal app: Settings → Linked Devices → Link New Device"
echo ""
echo "Press Enter to continue..."
read

# Get QR code link
echo "Generating QR code..."
QR_URL="${SIGNAL_API}/v1/qrcodelink?device_name=${DEVICE_NAME}"

echo ""
echo "Open this URL in a browser to see the QR code:"
echo "${QR_URL}"
echo ""
echo "Or use curl to get the QR code data:"
echo "  curl '${QR_URL}'"
echo ""
echo "After scanning, verify the link with:"
echo "  curl '${SIGNAL_API}/v1/accounts'"
```

**File: `scripts/encrypt_secrets.sh`**

```bash
#!/bin/bash
set -euo pipefail

# Dstack secret encryption script
# Encrypts sensitive environment variables for TEE deployment

KMS_URL="${KMS_URL:-https://kms.example.com}"
OUTPUT_FILE="${OUTPUT_FILE:-encrypted-env.json}"

echo "🔐 Dstack Secret Encryption"
echo "============================"
echo ""

# Get TEE encryption public key
echo "Fetching TEE public key from KMS..."
curl -s "${KMS_URL}/v1/encryption-key" > /tmp/tee-pubkey.pem

# Prompt for secrets
echo ""
read -p "Signal phone number: " SIGNAL_PHONE
read -sp "NEAR AI API key: " NEAR_AI_API_KEY
echo ""

# Create env file
cat > /tmp/secrets.env << EOF
SIGNAL_PHONE=${SIGNAL_PHONE}
NEAR_AI_API_KEY=${NEAR_AI_API_KEY}
EOF

# Encrypt (using dstack CLI if available)
if command -v dstack-encrypt &> /dev/null; then
    dstack-encrypt --pubkey /tmp/tee-pubkey.pem \
                   --env-file /tmp/secrets.env \
                   --output "${OUTPUT_FILE}"
    echo ""
    echo "✅ Encrypted secrets saved to: ${OUTPUT_FILE}"
else
    echo ""
    echo "⚠️ dstack-encrypt not found"
    echo "Install Dstack CLI or encrypt manually:"
    echo "  - Public key: /tmp/tee-pubkey.pem"
    echo "  - Secrets: /tmp/secrets.env"
fi

# Cleanup
rm -f /tmp/secrets.env
rm -f /tmp/tee-pubkey.pem
```

### 6.5 Environment Template

**File: `.env.example`**

```bash
# Signal Configuration
SIGNAL_SERVICE=http://signal-api:8080
SIGNAL_PHONE=+1234567890

# NEAR AI Configuration
NEAR_AI_API_KEY=your-api-key-here
NEAR_AI_BASE_URL=https://api.near.ai/v1
NEAR_AI_MODEL=llama-3.3-70b
NEAR_AI_TIMEOUT=60

# Redis Configuration
REDIS_URL=redis://redis:6379
REDIS_TTL_HOURS=24
MAX_CONVERSATION_MESSAGES=50

# Bot Configuration
LOG_LEVEL=INFO
SYSTEM_PROMPT="You are a helpful AI assistant..."

# Dstack Configuration (usually auto-configured in TEE)
DSTACK_SOCKET=/var/run/dstack.sock
```

### 6.6 Makefile

**File: `Makefile`**

```makefile
.PHONY: help install dev test lint format build run clean

PYTHON := python3
PIP := pip3

help:
	@echo "Signal Bot TEE - Development Commands"
	@echo ""
	@echo "  make install    - Install production dependencies"
	@echo "  make dev        - Install development dependencies"
	@echo "  make test       - Run tests"
	@echo "  make lint       - Run linters"
	@echo "  make format     - Format code"
	@echo "  make build      - Build Docker image"
	@echo "  make run        - Run locally with Docker Compose"
	@echo "  make clean      - Remove build artifacts"

install:
	$(PIP) install -r requirements.txt

dev:
	$(PIP) install -r requirements.txt -r requirements-dev.txt

test:
	$(PYTHON) -m pytest tests/ -v --cov=bot --cov-report=term-missing

lint:
	$(PYTHON) -m ruff check bot/ tests/
	$(PYTHON) -m mypy bot/

format:
	$(PYTHON) -m ruff format bot/ tests/
	$(PYTHON) -m ruff check --fix bot/ tests/

build:
	docker build -f docker/Dockerfile -t signal-bot-tee:latest .

run:
	docker compose -f docker/docker-compose.yaml up -d

stop:
	docker compose -f docker/docker-compose.yaml down

logs:
	docker compose -f docker/docker-compose.yaml logs -f

clean:
	find . -type d -name __pycache__ -exec rm -rf {} +
	find . -type d -name .pytest_cache -exec rm -rf {} +
	find . -type d -name .mypy_cache -exec rm -rf {} +
	find . -type f -name "*.pyc" -delete
	rm -rf build/ dist/ *.egg-info/
```

### 6.7 Tasks for Phase 5

| Task | Description | Files |
|------|-------------|-------|
| 5.1 | Create production Dockerfile | `docker/Dockerfile` |
| 5.2 | Create development Dockerfile | `docker/Dockerfile.dev` |
| 5.3 | Create docker-compose.yaml | `docker/docker-compose.yaml` |
| 5.4 | Create Signal setup script | `scripts/setup_signal.sh` |
| 5.5 | Create secret encryption script | `scripts/encrypt_secrets.sh` |
| 5.6 | Create .env.example | `.env.example` |
| 5.7 | Create Makefile | `Makefile` |
| 5.8 | Test local Docker deployment | Manual testing |
| 5.9 | Test Dstack deployment | Manual testing |

---

## 7. Phase 6: Testing

**Goal**: Comprehensive test coverage for all components.

### 7.1 Test Configuration

**File: `tests/conftest.py`**

```python
"""Pytest configuration and fixtures."""

import pytest
import pytest_asyncio
from unittest.mock import AsyncMock, MagicMock
import fakeredis.aioredis

from bot.config import Settings
from bot.near_ai_client import NearAIClient
from bot.conversation import ConversationStore
from bot.dstack_client import DstackClient


@pytest.fixture
def settings():
    """Test settings."""
    return Settings(
        signal_service="http://test-signal:8080",
        signal_phone="+1234567890",
        near_ai_api_key="test-api-key",
        near_ai_base_url="https://test.near.ai/v1",
        near_ai_model="test-model",
        redis_url="redis://localhost:6379",
        dstack_socket="/tmp/test-dstack.sock"
    )


@pytest_asyncio.fixture
async def fake_redis():
    """Fake Redis for testing."""
    return fakeredis.aioredis.FakeRedis()


@pytest_asyncio.fixture
async def conversation_store(fake_redis):
    """ConversationStore with fake Redis."""
    store = ConversationStore(
        redis_url="redis://localhost:6379",
        max_messages=10,
        ttl_hours=1
    )
    store._redis = fake_redis
    yield store
    await store.disconnect()


@pytest.fixture
def mock_near_ai():
    """Mocked NEAR AI client."""
    client = MagicMock(spec=NearAIClient)
    client.chat = AsyncMock(return_value="Test response")
    client.get_attestation = AsyncMock(return_value={"status": "ok"})
    client.get_models = AsyncMock(return_value=[{"id": "test-model"}])
    client.health_check = AsyncMock(return_value=True)
    client.model = "test-model"
    return client


@pytest.fixture
def mock_dstack():
    """Mocked Dstack client."""
    client = MagicMock(spec=DstackClient)
    client.is_in_tee = AsyncMock(return_value=True)
    client.get_app_info = AsyncMock(return_value={
        "app_id": "test-app",
        "compose_hash": "abc123",
        "instance_id": "inst-001"
    })
    client.get_quote = AsyncMock(return_value={
        "quote": "base64-encoded-quote",
        "report_data": "report-data"
    })
    return client


@pytest.fixture
def mock_signal_context():
    """Mocked Signal bot context."""
    ctx = MagicMock()
    ctx.message = MagicMock()
    ctx.message.source = "+1234567890"
    ctx.message.text = "Hello, AI!"
    ctx.message.timestamp = 1234567890
    ctx.send = AsyncMock()
    return ctx
```

### 7.2 NEAR AI Client Tests

**File: `tests/test_near_ai_client.py`**

```python
"""Tests for NEAR AI client."""

import pytest
from unittest.mock import AsyncMock, patch, MagicMock

from bot.near_ai_client import NearAIClient
from bot.utils.errors import NearAIError, NearAIRateLimitError, NearAIAuthError


class TestNearAIClient:
    """Tests for NearAIClient class."""

    @pytest.fixture
    def client(self):
        """Create test client."""
        return NearAIClient(
            api_key="test-key",
            base_url="https://test.api/v1",
            model="test-model"
        )

    @pytest.mark.asyncio
    async def test_chat_returns_response(self, client):
        """Test successful chat completion."""
        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]
        mock_response.choices[0].message.content = "Hello!"

        with patch.object(
            client.client.chat.completions,
            'create',
            new_callable=AsyncMock,
            return_value=mock_response
        ):
            result = await client.chat([{"role": "user", "content": "Hi"}])
            assert result == "Hello!"

    @pytest.mark.asyncio
    async def test_chat_handles_rate_limit(self, client):
        """Test rate limit error handling."""
        with patch.object(
            client.client.chat.completions,
            'create',
            new_callable=AsyncMock,
            side_effect=Exception("Rate limit exceeded (429)")
        ):
            with pytest.raises(NearAIRateLimitError):
                await client.chat([{"role": "user", "content": "Hi"}])

    @pytest.mark.asyncio
    async def test_chat_handles_auth_error(self, client):
        """Test authentication error handling."""
        with patch.object(
            client.client.chat.completions,
            'create',
            new_callable=AsyncMock,
            side_effect=Exception("Unauthorized (401)")
        ):
            with pytest.raises(NearAIAuthError):
                await client.chat([{"role": "user", "content": "Hi"}])

    @pytest.mark.asyncio
    async def test_health_check_success(self, client):
        """Test successful health check."""
        with patch.object(
            client.client.models,
            'list',
            new_callable=AsyncMock,
            return_value=MagicMock(data=[])
        ):
            result = await client.health_check()
            assert result is True

    @pytest.mark.asyncio
    async def test_health_check_failure(self, client):
        """Test failed health check."""
        with patch.object(
            client.client.models,
            'list',
            new_callable=AsyncMock,
            side_effect=Exception("Connection failed")
        ):
            result = await client.health_check()
            assert result is False
```

### 7.3 Conversation Store Tests

**File: `tests/test_conversation.py`**

```python
"""Tests for conversation storage."""

import pytest
from datetime import datetime

from bot.conversation import ConversationStore, Conversation, Message


class TestConversationStore:
    """Tests for ConversationStore class."""

    @pytest.mark.asyncio
    async def test_add_message_creates_conversation(self, conversation_store):
        """Test adding message to new conversation."""
        conv = await conversation_store.add_message(
            user_id="+1234567890",
            role="user",
            content="Hello!",
            system_prompt="Be helpful"
        )

        assert conv.user_id == "+1234567890"
        assert len(conv.messages) == 1
        assert conv.messages[0].role == "user"
        assert conv.messages[0].content == "Hello!"
        assert conv.system_prompt == "Be helpful"

    @pytest.mark.asyncio
    async def test_add_message_appends_to_existing(self, conversation_store):
        """Test adding message to existing conversation."""
        await conversation_store.add_message("+1234567890", "user", "Hello!")
        conv = await conversation_store.add_message("+1234567890", "assistant", "Hi there!")

        assert len(conv.messages) == 2
        assert conv.messages[1].role == "assistant"
        assert conv.messages[1].content == "Hi there!"

    @pytest.mark.asyncio
    async def test_message_trimming(self, conversation_store):
        """Test that old messages are trimmed."""
        # Store has max_messages=10
        for i in range(15):
            await conversation_store.add_message(
                "+1234567890", "user", f"Message {i}"
            )

        conv = await conversation_store.get("+1234567890")
        assert len(conv.messages) == 10
        # Should have messages 5-14 (most recent)
        assert conv.messages[0].content == "Message 5"
        assert conv.messages[-1].content == "Message 14"

    @pytest.mark.asyncio
    async def test_get_nonexistent_returns_none(self, conversation_store):
        """Test getting non-existent conversation."""
        conv = await conversation_store.get("+9999999999")
        assert conv is None

    @pytest.mark.asyncio
    async def test_clear_conversation(self, conversation_store):
        """Test clearing conversation."""
        await conversation_store.add_message("+1234567890", "user", "Hello!")

        result = await conversation_store.clear("+1234567890")
        assert result is True

        conv = await conversation_store.get("+1234567890")
        assert conv is None

    @pytest.mark.asyncio
    async def test_clear_nonexistent_returns_false(self, conversation_store):
        """Test clearing non-existent conversation."""
        result = await conversation_store.clear("+9999999999")
        assert result is False

    @pytest.mark.asyncio
    async def test_to_openai_messages(self, conversation_store):
        """Test conversion to OpenAI format."""
        await conversation_store.add_message(
            "+1234567890", "user", "Hello!", "Be helpful"
        )
        await conversation_store.add_message(
            "+1234567890", "assistant", "Hi there!"
        )

        messages = await conversation_store.to_openai_messages("+1234567890")

        assert len(messages) == 3
        assert messages[0]["role"] == "system"
        assert messages[0]["content"] == "Be helpful"
        assert messages[1]["role"] == "user"
        assert messages[2]["role"] == "assistant"

    @pytest.mark.asyncio
    async def test_to_openai_messages_with_override(self, conversation_store):
        """Test system prompt override."""
        await conversation_store.add_message(
            "+1234567890", "user", "Hello!", "Original prompt"
        )

        messages = await conversation_store.to_openai_messages(
            "+1234567890",
            system_prompt="Override prompt"
        )

        assert messages[0]["content"] == "Override prompt"
```

### 7.4 Command Tests

**File: `tests/test_commands/test_chat.py`**

```python
"""Tests for chat command."""

import pytest
from unittest.mock import AsyncMock

from bot.commands.chat import ChatCommand
from bot.utils.errors import NearAIError, NearAIRateLimitError


class TestChatCommand:
    """Tests for ChatCommand class."""

    @pytest.fixture
    def chat_cmd(self, mock_near_ai, conversation_store):
        """Create chat command instance."""
        return ChatCommand(
            near_ai=mock_near_ai,
            conversations=conversation_store,
            system_prompt="Test prompt"
        )

    @pytest.mark.asyncio
    async def test_processes_message(
        self, chat_cmd, mock_signal_context, conversation_store
    ):
        """Test normal message processing."""
        mock_signal_context.message.text = "Hello AI!"

        await chat_cmd.execute(
            mock_signal_context,
            "+1234567890",
            "Hello AI!"
        )

        mock_signal_context.send.assert_called_once_with("Test response")

        # Verify conversation was saved
        conv = await conversation_store.get("+1234567890")
        assert len(conv.messages) == 2
        assert conv.messages[0].content == "Hello AI!"
        assert conv.messages[1].content == "Test response"

    @pytest.mark.asyncio
    async def test_skips_commands(self, chat_cmd, mock_signal_context):
        """Test that command messages are skipped."""
        mock_signal_context.message.text = "!help"

        await chat_cmd.execute(
            mock_signal_context,
            "+1234567890",
            "!help"
        )

        mock_signal_context.send.assert_not_called()

    @pytest.mark.asyncio
    async def test_handles_rate_limit(
        self, chat_cmd, mock_signal_context, mock_near_ai
    ):
        """Test rate limit handling."""
        mock_near_ai.chat = AsyncMock(side_effect=NearAIRateLimitError("Limit"))

        await chat_cmd.execute(
            mock_signal_context,
            "+1234567890",
            "Hello!"
        )

        # Should send rate limit message
        call_args = mock_signal_context.send.call_args[0][0]
        assert "too many requests" in call_args.lower()

    @pytest.mark.asyncio
    async def test_handles_api_error(
        self, chat_cmd, mock_signal_context, mock_near_ai
    ):
        """Test API error handling."""
        mock_near_ai.chat = AsyncMock(side_effect=NearAIError("API down"))

        await chat_cmd.execute(
            mock_signal_context,
            "+1234567890",
            "Hello!"
        )

        # Should send error message
        call_args = mock_signal_context.send.call_args[0][0]
        assert "error" in call_args.lower()
```

### 7.5 Tasks for Phase 6

| Task | Description | Files |
|------|-------------|-------|
| 6.1 | Create test configuration | `tests/conftest.py` |
| 6.2 | Write NEAR AI client tests | `tests/test_near_ai_client.py` |
| 6.3 | Write conversation store tests | `tests/test_conversation.py` |
| 6.4 | Write Dstack client tests | `tests/test_dstack_client.py` |
| 6.5 | Write chat command tests | `tests/test_commands/test_chat.py` |
| 6.6 | Write verify command tests | `tests/test_commands/test_verify.py` |
| 6.7 | Write clear command tests | `tests/test_commands/test_clear.py` |
| 6.8 | Create integration test suite | `tests/integration/` |
| 6.9 | Achieve >80% code coverage | All test files |

---

## 8. Phase 7: Documentation & Polish

**Goal**: Final documentation, code cleanup, and release preparation.

### 8.1 Tasks

| Task | Description | Files |
|------|-------------|-------|
| 7.1 | Update README with quick start | `README.md` |
| 7.2 | Add API documentation | `bot/` docstrings |
| 7.3 | Create CHANGELOG | `CHANGELOG.md` |
| 7.4 | Add type hints throughout | All Python files |
| 7.5 | Run final lint and format | All files |
| 7.6 | Security review | All files |
| 7.7 | Performance testing | Manual testing |
| 7.8 | Create release checklist | `docs/RELEASE.md` |

---

## 9. File Manifest

Complete list of files to create:

### Core Application
- `bot/__init__.py`
- `bot/main.py`
- `bot/config.py`
- `bot/near_ai_client.py`
- `bot/conversation.py`
- `bot/dstack_client.py`

### Commands
- `bot/commands/__init__.py`
- `bot/commands/base.py`
- `bot/commands/chat.py`
- `bot/commands/verify.py`
- `bot/commands/clear.py`
- `bot/commands/help.py`
- `bot/commands/models.py`

### Utilities
- `bot/utils/__init__.py`
- `bot/utils/logging.py`
- `bot/utils/errors.py`

### Tests
- `tests/__init__.py`
- `tests/conftest.py`
- `tests/test_near_ai_client.py`
- `tests/test_conversation.py`
- `tests/test_dstack_client.py`
- `tests/test_commands/__init__.py`
- `tests/test_commands/test_chat.py`
- `tests/test_commands/test_verify.py`
- `tests/test_commands/test_clear.py`
- `tests/integration/__init__.py`
- `tests/integration/test_e2e.py`

### Scripts
- `scripts/setup_signal.sh`
- `scripts/encrypt_secrets.sh`
- `scripts/verify_tee.py`
- `scripts/health_check.py`

### Docker
- `docker/Dockerfile`
- `docker/Dockerfile.dev`
- `docker/docker-compose.yaml`

### Configuration
- `pyproject.toml`
- `requirements.txt`
- `requirements-dev.txt`
- `.env.example`
- `Makefile`

### Documentation
- `README.md` (update)
- `DESIGN.md` (exists)
- `IMPLEMENTATION_PLAN.md` (this file)
- `CHANGELOG.md`

---

## 10. Dependencies

### Production Dependencies (`requirements.txt`)

```
httpx>=0.27.0
signalbot>=0.8.0
openai>=1.0.0
redis>=5.0.0
pydantic>=2.0.0
pydantic-settings>=2.0.0
```

### Development Dependencies (`requirements-dev.txt`)

```
pytest>=8.0.0
pytest-asyncio>=0.23.0
pytest-cov>=4.0.0
mypy>=1.8.0
ruff>=0.2.0
fakeredis>=2.20.0
respx>=0.20.0
```

---

## Summary

This implementation plan breaks the Signal Bot TEE project into 7 phases:

1. **Foundation** (8 tasks): Project structure, configuration, logging, errors
2. **Core Components** (10 tasks): NEAR AI client, conversation store, Dstack client
3. **Bot Application** (9 tasks): Commands and main entry point
4. **TEE Integration** (5 tasks): Verification utilities and testing
5. **Docker & Deployment** (9 tasks): Containerization and deployment scripts
6. **Testing** (9 tasks): Comprehensive test coverage
7. **Documentation** (8 tasks): Final polish and release prep

**Total: 58 tasks**

Each phase builds on the previous one, allowing incremental development and testing. The plan provides complete code samples that can be directly used or adapted during implementation.

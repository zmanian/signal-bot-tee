# Build stage
FROM rust:1.83-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/signal-bot/Cargo.toml crates/signal-bot/
COPY crates/near-ai-client/Cargo.toml crates/near-ai-client/
COPY crates/conversation-store/Cargo.toml crates/conversation-store/
COPY crates/dstack-client/Cargo.toml crates/dstack-client/
COPY crates/signal-client/Cargo.toml crates/signal-client/
COPY crates/signal-registration-proxy/Cargo.toml crates/signal-registration-proxy/
COPY crates/tools/Cargo.toml crates/tools/

# Create dummy source files for dependency caching
RUN mkdir -p crates/signal-bot/src/commands && \
    echo "pub mod commands; pub mod config; pub mod error;" > crates/signal-bot/src/lib.rs && \
    echo "fn main() {}" > crates/signal-bot/src/main.rs && \
    echo "" > crates/signal-bot/src/commands/mod.rs && \
    mkdir -p crates/near-ai-client/src && echo "pub fn dummy() {}" > crates/near-ai-client/src/lib.rs && \
    mkdir -p crates/conversation-store/src && echo "pub fn dummy() {}" > crates/conversation-store/src/lib.rs && \
    mkdir -p crates/dstack-client/src && echo "pub fn dummy() {}" > crates/dstack-client/src/lib.rs && \
    mkdir -p crates/signal-client/src && echo "pub fn dummy() {}" > crates/signal-client/src/lib.rs && \
    mkdir -p crates/signal-registration-proxy/src && \
    echo "pub mod api; pub mod config; pub mod error; pub mod registry; pub mod signal;" > crates/signal-registration-proxy/src/lib.rs && \
    echo "fn main() {}" > crates/signal-registration-proxy/src/main.rs && \
    mkdir -p crates/tools/src && echo "pub fn dummy() {}" > crates/tools/src/lib.rs

# Build dependencies only (ignore errors from dummy files)
RUN cargo build --release 2>/dev/null || true

# Copy actual source
COPY crates crates

# Touch source files to trigger rebuild
RUN find crates -name "*.rs" -exec touch {} \;

# Build release binary
RUN cargo build --release --bin signal-bot

# Runtime stage
FROM alpine:3.21

# Install CA certificates for HTTPS
RUN apk add --no-cache ca-certificates

# Copy binary
COPY --from=builder /app/target/release/signal-bot /signal-bot

# Non-root user
RUN adduser -D -u 1000 appuser
USER appuser

ENTRYPOINT ["/signal-bot"]

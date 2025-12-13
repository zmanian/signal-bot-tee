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

# Create dummy source files for dependency caching
RUN mkdir -p crates/signal-bot/src/commands && \
    echo "fn main() {}" > crates/signal-bot/src/main.rs && \
    echo "" > crates/signal-bot/src/commands/mod.rs && \
    mkdir -p crates/near-ai-client/src && echo "pub fn dummy() {}" > crates/near-ai-client/src/lib.rs && \
    mkdir -p crates/conversation-store/src && echo "pub fn dummy() {}" > crates/conversation-store/src/lib.rs && \
    mkdir -p crates/dstack-client/src && echo "pub fn dummy() {}" > crates/dstack-client/src/lib.rs && \
    mkdir -p crates/signal-client/src && echo "pub fn dummy() {}" > crates/signal-client/src/lib.rs

# Build dependencies only
RUN cargo build --release --target x86_64-unknown-linux-musl 2>/dev/null || true

# Copy actual source
COPY crates crates

# Touch source files to trigger rebuild
RUN find crates -name "*.rs" -exec touch {} \;

# Build release binary
RUN cargo build --release --target x86_64-unknown-linux-musl --bin signal-bot

# Runtime stage
FROM scratch

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/signal-bot /signal-bot

# Non-root user (UID 1000)
USER 1000

ENTRYPOINT ["/signal-bot"]

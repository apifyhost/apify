# Multi-stage build for minimal image size
# Stage 1: Build stage
FROM rust:slim-trixie AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests (workspace root and all members)
COPY Cargo.toml Cargo.lock ./
COPY apify/Cargo.toml ./apify/
COPY sdk/Cargo.toml ./sdk/
COPY flow/Cargo.toml ./flow/
COPY asd/Cargo.toml ./asd/
COPY runtime/Cargo.toml ./runtime/
COPY plugins/http_server/Cargo.toml ./plugins/http_server/
COPY plugins/echo/Cargo.toml ./plugins/echo/
COPY plugins/amqp/Cargo.toml ./plugins/amqp/
COPY plugins/log/Cargo.toml ./plugins/log/
COPY plugins/sleep/Cargo.toml ./plugins/sleep/
COPY plugins/http_request/Cargo.toml ./plugins/http_request/
COPY plugins/postgres/Cargo.toml ./plugins/postgres/
COPY plugins/cli/Cargo.toml ./plugins/cli/
COPY plugins/rpc/Cargo.toml ./plugins/rpc/
COPY plugins/jwt/Cargo.toml ./plugins/jwt/
COPY plugins/cache/Cargo.toml ./plugins/cache/

# Create dummy source files to cache dependencies
RUN mkdir -p apify/src sdk/src flow/src asd/src runtime/src \
    plugins/http_server/src plugins/echo/src plugins/amqp/src \
    plugins/log/src plugins/sleep/src plugins/http_request/src \
    plugins/postgres/src plugins/cli/src plugins/rpc/src \
    plugins/jwt/src plugins/cache/src && \
    echo "fn main() {}" > apify/src/main.rs && \
    echo "pub fn dummy() {}" > apify/src/lib.rs && \
    echo "pub fn dummy() {}" > sdk/src/lib.rs && \
    echo "pub fn dummy() {}" > flow/src/lib.rs && \
    echo "pub fn dummy() {}" > asd/src/lib.rs && \
    echo "fn main() {}" > runtime/src/main.rs && \
    for plugin in http_server echo amqp log sleep http_request postgres cli rpc jwt cache; do \
        echo "pub fn dummy() {}" > plugins/$plugin/src/lib.rs; \
    done

# Build dependencies (this layer will be cached)
RUN cargo build --release --package apify

# Remove dummy build artifacts (keep dependencies)
RUN rm -rf target/release/.fingerprint/apify-* \
    target/release/deps/apify-* \
    target/release/deps/libapify-* && \
    rm -rf apify/src/*

# Copy only apify source code (the only one we need)
COPY apify/src ./apify/src

# Build the actual application
RUN cargo build --release --package apify

# Stage 2: Runtime stage with minimal Ubuntu
FROM ubuntu:24.04

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user (use --force to avoid UID conflict)
RUN useradd -m -u 1000 apify 2>/dev/null || useradd -m apify

# Set working directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/apify /usr/local/bin/apify

# Copy default config directory structure
RUN mkdir -p /app/config /app/data && \
    chown -R apify:apify /app

# Switch to non-root user
USER apify

# Expose default port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/apify", "--version"] || exit 1

# Default command
ENTRYPOINT ["/usr/local/bin/apify"]
CMD ["-c", "/app/config/config.yaml"]

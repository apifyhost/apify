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

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY apify/Cargo.toml ./apify/
COPY sdk/Cargo.toml ./sdk/
COPY flow/Cargo.toml ./flow/
COPY asd/Cargo.toml ./asd/
COPY runtime/Cargo.toml ./runtime/

# Create dummy source files to cache dependencies
RUN mkdir -p apify/src sdk/src flow/src asd/src runtime/src && \
    echo "fn main() {}" > apify/src/main.rs && \
    echo "pub fn dummy() {}" > apify/src/lib.rs && \
    echo "pub fn dummy() {}" > sdk/src/lib.rs && \
    echo "pub fn dummy() {}" > flow/src/lib.rs && \
    echo "pub fn dummy() {}" > asd/src/lib.rs && \
    echo "fn main() {}" > runtime/src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --package apify

# Remove dummy build artifacts
RUN rm -rf target/release/.fingerprint/apify-* \
    target/release/.fingerprint/sdk-* \
    target/release/.fingerprint/flow-* \
    target/release/.fingerprint/asd-* \
    target/release/deps/apify-* \
    target/release/deps/libapify-* \
    target/release/deps/libsdk-* \
    target/release/deps/libflow-* \
    target/release/deps/libasd-*

# Copy actual source code
COPY apify/src ./apify/src
COPY sdk/src ./sdk/src
COPY flow/src ./flow/src
COPY asd/src ./asd/src
COPY runtime/src ./runtime/src

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

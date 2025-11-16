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

# Create dummy source files to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release

# Remove dummy build artifacts (keep dependencies)
RUN rm -rf target/release/.fingerprint/apify-* \
    target/release/deps/apify-* \
    target/release/deps/libapify-* && \
    rm -rf src/*

# Copy actual source code
COPY src ./src

# Build the actual application
RUN cargo build --release

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

# Use CMD instead of ENTRYPOINT to allow easy override
CMD ["/usr/local/bin/apify", "-c", "/app/config/config.yaml"]

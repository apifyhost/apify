# Docker Deployment Guide

This guide explains how to build and deploy Apify using Docker.

## Quick Start

### Using Docker Compose (Recommended)

Run with SQLite:
```bash
docker compose up apify-sqlite
```

Run with PostgreSQL (Split Control Plane and Data Plane):
```bash
docker compose up postgres apify-cp apify-dp
```

### Using Docker CLI

Build the image:
```bash
docker build -t apify:latest .
```

Run with SQLite:
```bash
docker run -d \
  --name apify \
  -p 3000:3000 \
  -v $(pwd)/apify/config:/app/config:ro \
  -v apify-data:/app/data \
  apify:latest
```

Run with PostgreSQL:
```bash
docker run -d \
  --name apify \
  -p 3000:3000 \
  -v $(pwd)/apify/config:/app/config:ro \
  -e POSTGRES_HOST=your-postgres-host \
  apify:latest
```

### Control Plane and Data Plane Separation

You can run Control Plane and Data Plane as separate services for better scalability and security.

```bash
# Run Control Plane
docker run -d \
  --name apify-cp \
  -p 4000:4000 \
  -v $(pwd)/config:/app/config:ro \
  -v apify-metadata:/app/data \
  apify:latest \
  apify --control-plane -c /app/config/config.yaml

# Run Data Plane
docker run -d \
  --name apify-dp \
  -p 3000:3000 \
  -v $(pwd)/config:/app/config:ro \
  -v apify-metadata:/app/data \
  apify:latest \
  apify --data-plane -c /app/config/config.yaml
```

## Image Details

- **Base Image**: Ubuntu 24.04 (minimal)
- **Size**: ~150MB (optimized multi-stage build)
- **Platforms**: linux/amd64, linux/arm64 (for releases)
- **User**: Runs as non-root user `apify` (UID 1000)

## Environment Variables

- `RUST_LOG` - Set log level (error, warn, info, debug, trace)
- `APIFY_THREADS` - Number of worker threads (default: 2)

## Volumes

- `/app/config` - Configuration files (mount your config directory here)
- `/app/data` - Data directory for SQLite databases

## Health Check

The container includes a health check that runs every 30 seconds:
```bash
docker inspect --format='{{.State.Health.Status}}' apify
```

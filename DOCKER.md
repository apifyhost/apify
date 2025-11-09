# Docker Deployment Guide

This guide explains how to build and deploy Apify using Docker.

## Quick Start

### Using Docker Compose (Recommended)

Run with SQLite:
```bash
docker-compose up apify-sqlite
```

Run with PostgreSQL:
```bash
docker-compose up postgres apify-postgres
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

## E2E Testing

Run the full E2E test suite:

```bash
# Test with SQLite
docker-compose up -d apify-sqlite
./e2e/test.sh

# Test with PostgreSQL
docker-compose up -d postgres apify-postgres
BASE_URL=http://localhost:3001 ./e2e/test.sh
```

## CI/CD

The project includes GitHub Actions workflows that:
1. Build the Docker image
2. Run E2E tests with SQLite
3. Run E2E tests with PostgreSQL
4. Scan for security vulnerabilities
5. Push to GitHub Container Registry (ghcr.io)

### Pull Pre-built Images

```bash
# Latest from main branch
docker pull ghcr.io/apifyhost/apify:latest

# Specific version
docker pull ghcr.io/apifyhost/apify:v1.0.0
```

## Production Deployment

### Docker Compose Production Setup

Create a `docker-compose.prod.yml`:

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: ${POSTGRES_USER}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: ${POSTGRES_DB}
    volumes:
      - postgres_data:/var/lib/postgresql/data
    restart: unless-stopped

  apify:
    image: ghcr.io/apifyhost/apify:latest
    ports:
      - "3000:3000"
    volumes:
      - ./config:/app/config:ro
    environment:
      - RUST_LOG=info
      - APIFY_THREADS=4
    depends_on:
      - postgres
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/healthz"]
      interval: 30s
      timeout: 3s
      retries: 3

volumes:
  postgres_data:
```

Deploy:
```bash
docker-compose -f docker-compose.prod.yml up -d
```

## Troubleshooting

### View logs
```bash
docker logs apify
docker logs -f apify  # Follow logs
```

### Enter container
```bash
docker exec -it apify /bin/bash
```

### Check health
```bash
curl http://localhost:3000/healthz
```

### Debug database connection
```bash
# For PostgreSQL
docker exec -it apify env | grep POSTGRES
```

## Security

- Runs as non-root user (UID 1000)
- Minimal attack surface (Ubuntu slim base)
- Regular security scans via Trivy in CI
- No unnecessary packages installed

## Performance

The multi-stage build:
1. Caches Rust dependencies separately
2. Only rebuilds when source code changes
3. Produces minimal runtime image
4. Supports parallel builds with BuildKit

Build with cache:
```bash
docker build --cache-from apify:latest -t apify:latest .
```

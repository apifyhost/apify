# Quick Start Guide

## Recommended Method

The fastest way to get Apify running is using the quickstart script:

```bash
# Download and run the quickstart script
curl -fsSL https://raw.githubusercontent.com/apifyhost/apify/main/quickstart.sh | bash
```

The quickstart script will:
- ✅ Download and extract all necessary files
- ✅ Pull the Docker image
- ✅ Start Apify with SQLite
- ✅ Display access URLs and quick commands

### Quickstart Commands

```bash
./quickstart.sh install   # Download and install (default)
./quickstart.sh start     # Start services
./quickstart.sh stop      # Stop services
./quickstart.sh status    # Check service status
./quickstart.sh destroy   # Remove installation
```

## Manual Docker Setup

If you prefer to run Docker manually:

```bash
# Pull the latest image
docker pull apifyhost/apify:latest

# Run with SQLite
docker run -d \
  -p 3000:3000 \
  -v $(pwd)/config:/app/config:ro \
  -v apify-data:/app/data \
  apifyhost/apify:latest
```

For more details, see the [Docker Guide](docker.md).

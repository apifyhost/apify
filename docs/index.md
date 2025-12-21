# Apify

**Make everything as API** - A flexible, high-performance API framework that automatically generates CRUD operations from OpenAPI specifications.

[![Docker](https://img.shields.io/badge/docker-latest-blue)](https://hub.docker.com/r/apifyhost/apify)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

---

## 🚀 Quick Start

The fastest way to get Apify running is using our quickstart script:

```bash
curl -fsSL https://raw.githubusercontent.com/apifyhost/apify/main/quickstart.sh | bash
```

This will:
1. Download the necessary files
2. Start Apify with a sample configuration
3. Provide you with access URLs

For more details, see the [Quick Start Guide](getting-started/quickstart.md).

---

## 🌟 Key Features

### 🚀 Zero-Code CRUD
Define your data models in OpenAPI specs with `x-table-schemas`, and Apify automatically generates complete CRUD endpoints (Create, Read, Update, Delete) with database operations.

### 🗄️ Multi-Database Support
Support for **SQLite** and **PostgreSQL** backends with automatic connection pooling and schema initialization.

### 🔐 Built-in Security
- **API Key** authentication
- **OAuth 2.0 / OIDC** support
- **Automatic Audit Trail** for tracking changes
- Granular access control

### ⚡ High Performance
Built on Rust and Tokio, featuring multi-threaded architecture and zero-copy request routing.

---

## 📚 Documentation

Full documentation is available at [https://apifyhost.github.io/apify/](https://apifyhost.github.io/apify/).

- [Getting Started](getting-started/quickstart.md)
- [Configuration Guide](configuration/main-config.md)
- [Architecture Overview](concepts/architecture.md)

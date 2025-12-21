# Apify

**Turn your Database into a REST API in seconds.**

Apify is a high-performance, zero-code API gateway that instantly generates RESTful APIs from your database schema using OpenAPI definitions.

---

## 🚀 Features

*   **Zero-Code**: Define APIs using standard OpenAPI (Swagger) files.
*   **Database Support**: Works with PostgreSQL, MySQL, SQLite.
*   **High Performance**: Built with Rust for speed and safety.
*   **Security**: Built-in API Key authentication, OAuth2/OIDC support.
*   **Observability**: Integrated Prometheus metrics and OpenTelemetry tracing.
*   **Audit Trail**: Comprehensive request logging for security and compliance.

## 📚 Documentation

Full documentation is available at **[https://phlow.dev](https://phlow.dev)** (or your configured GitHub Pages URL).

*   [Quick Start](https://phlow.dev/getting-started/quickstart/)
*   [Installation](https://phlow.dev/getting-started/installation/)
*   [Configuration](https://phlow.dev/configuration/main-config/)

## ⚡️ Quick Start

### 1. Start the Server

The easiest way to get started is using the provided quickstart script, which sets up a complete environment with Docker Compose.

```bash
./quickstart.sh
```

This will start:
*   Apify server on port 8080
*   A sample SQLite database
*   Keycloak (for OAuth examples)
*   Prometheus & Grafana (for observability examples)

### 2. Test the API

Once running, you can interact with the sample API:

```bash
# Create a user
curl -X POST http://localhost:8080/users \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "email": "alice@example.com"}'

# List users
curl http://localhost:8080/users
```

### 3. Explore Examples

The `quickstart.sh` script supports different modes:

```bash
./quickstart.sh basic          # Basic CRUD (default)
./quickstart.sh oauth          # With Keycloak authentication
./quickstart.sh observability  # With Prometheus & Grafana
./quickstart.sh full           # All features enabled
```

See `examples/` directory for configuration details.


## 🛠️ Build from Source

```bash
cargo build --release
./target/release/apify
```

## 📄 License

MIT

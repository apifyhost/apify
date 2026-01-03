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

Full documentation is available at **[https://docs.apify.host/](https://docs.apify.host/)**.

*   [Quick Start](https://apifyhost.github.io/apify//getting-started/quickstart/)
*   [Installation](https://apifyhost.github.io/apify//getting-started/installation/)
*   [Configuration](https://apifyhost.github.io/apify//configuration/main-config/)

## ⚡️ Quick Start

### 1. Start the Server

The easiest way to get started is using the provided quickstart script, which sets up a complete environment with Docker Compose.

```bash
curl -fsSL https://raw.githubusercontent.com/apifyhost/apify/main/quickstart.sh | bash
```

This will start:
*   Apify Control Plane server on port 4000
*   Apify Data Plane server on port 3000
*   A sample Postgres database
*   Keycloak (for OAuth examples)
*   Prometheus & Grafana (for observability examples)

### 2. Configure the API

Apify is fully dynamic. You can configure APIs and Listeners at runtime using the Control Plane API.

**1. Add a Datasource:**

```bash
curl -X POST http://localhost:4000/_meta/datasources \
  -H "Content-Type: application/json" \
  -d '{
    "name": "postgres",
    "config": {
      "driver": "postgres",
      "host": "postgres",
      "port": 5432,
      "user": "apify",
      "password": "apify_password",
      "database": "apify_db"
    }
  }'
```

**2. Create an API Definition:**

Register the API by providing the OpenAPI specification directly.

```bash
curl -X POST http://localhost:4000/_meta/apis \
  -H "Content-Type: application/json" \
  -d '{
    "name": "users",
    "version": "1.0.0",
    "datasource_name": "postgres",
    "spec": {
      "openapi": "3.0.0",
      "info": {
        "title": "Users API",
        "version": "1.0.0"
      },
      "paths": {
        "/users": {
          "get": {
            "x-table-name": "users"
          },
          "post": {
            "x-table-name": "users"
          }
        }
      },
      "components": {
        "schemas": {
          "User": {
            "type": "object",
            "properties": {
              "id": { "type": "integer", "format": "int64" },
              "name": { "type": "string" },
              "email": { "type": "string" }
            },
            "required": ["name", "email"]
          }
        }
      }
    }
  }'
```

**3. Create a Listener:**

Expose the API on port 3000.

```bash
curl -X POST http://localhost:4000/_meta/listeners \
  -H "Content-Type: application/json" \
  -d '{
    "name": "main-listener",
    "port": 3000,
    "ip": "0.0.0.0",
    "protocol": "http",
    "apis": ["users"]
  }'
```

### 3. Test the API

Now your API is live!

```bash
# Create a user
curl -X POST http://localhost:3000/users \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "email": "alice@example.com"}'

# List users
curl http://localhost:3000/users
```

### 3. Explore Examples

To run other examples (OAuth, Observability, etc.), clone the repository and use the script locally:

```bash
git clone https://github.com/apifyhost/apify.git
cd apify
./quickstart.sh oauth          # With Keycloak authentication
./quickstart.sh observability  # With Prometheus & Grafana
```

See `examples/` directory for configuration details.


## 🛠️ Build from Source

```bash
cargo build --release
./target/release/apify
```

## 📄 License

MIT

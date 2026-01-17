# Apify

**Turn your Database into a REST API in seconds.**

Apify is a high-performance, zero-code API gateway that instantly generates RESTful APIs from your database schema using OpenAPI definitions.

[English](#) | [中文](./README.zh-CN.md)

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

**1. Create a Listener:**

Expose an HTTP server on port 3000. We give it a name (`main-listener`) to reference it later.

```bash
curl -X POST http://localhost:4000/_meta/listeners \
  -H "X-API-KEY: UZY65Nakvsd3" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "main-listener",
    "port": 3000,
    "ip": "0.0.0.0",
    "protocol": "http"
  }'
```

**2. Add a Datasource:**

```bash
curl -X POST http://localhost:4000/_meta/datasources \
  -H "X-API-KEY: UZY65Nakvsd3" \
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

**3. Create an API Definition:**

Register the API, link it to the datasource and the listener.

> **Note**: Configuration changes may take a few seconds to propagate (default polling interval is 10s).

```bash
curl -X POST http://localhost:4000/_meta/apis \
  -H "X-API-KEY: UZY65Nakvsd3" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "users",
    "version": "1.0.0",
    "datasource_name": "postgres",
    "listeners": ["main-listener"],
    "spec": {
      "openapi": "3.0.0",
      "info": {
        "title": "Users API",
        "version": "1.0.0"
      },
      "paths": {
        "/users": {
          "get": {
            "summary": "List users",
            "responses": {
              "200": {
                "description": "List of users",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": {
                        "$ref": "#/components/schemas/User"
                      }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "summary": "Create a user",
            "requestBody": {
              "content": {
                "application/json": {
                  "schema": {
                    "$ref": "#/components/schemas/User"
                  }
                }
              }
            },
            "responses": {
              "201": {
                "description": "User created"
              }
            }
          }
        }
      },
      "components": {
        "schemas": {
          "User": {
            "type": "object",
            "properties": {
              "id": { "type": "integer", "format": "int64", "readOnly": true },
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

### 3. Test the API

Now your API is live! (Please wait up to 10 seconds for the configuration to reload)

```bash
# Create a user
curl -X POST http://localhost:3000/users \
  -H "X-API-KEY: UZY65Nakvsd3" \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "email": "alice@example.com"}'

# List users
curl -H "X-API-KEY: UZY65Nakvsd3" http://localhost:3000/users
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

## 📘 Documentation Server (Swagger UI)

Apify includes a built-in Documentation Server that aggregates all your configured APIs into a single OpenAPI specification and serves a Swagger UI.

This service is distinct from your main API listeners.

*   **Swagger UI**: [http://localhost:4001/docs](http://localhost:4001/docs)
*   **OpenAPI Spec**: [http://localhost:4001/openapi.json](http://localhost:4001/openapi.json)

### Configuration

The documentation server is **disabled by default**. To enable it, you must configure the `modules.openapi_docs` section in your `config.yaml`:

```yaml
modules:
  openapi_docs:
    enabled: true
    port: 4001
```

> **Note:** The [Quickstart](#-quick-start) script uses a configuration that enables this feature on port 4001 by default.

## 🛠️ Build from Source

```bash
cargo build --release
./target/release/apify
```

## 📄 License

MIT

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

1.  **Run with Docker**:
    ```bash
    docker run -p 8080:8080 -v $(pwd)/config:/app/config apify/apify
    ```

2.  **Define your API**:
    Create an OpenAPI file in `config/openapi/` and add `x-apify-action` extensions.

3.  **Query**:
    ```bash
    curl http://localhost:8080/users
    ```

## 🛠️ Build from Source

```bash
cargo build --release
./target/release/apify
```

## 📄 License

MIT

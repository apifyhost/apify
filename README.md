# Apify

Make everything as API - A flexible API framework with auto-generated CRUD operations from OpenAPI specifications.

## Features

- 🚀 **Auto-generated CRUD** from OpenAPI specs
- 🗄️ **Multi-database support** (SQLite, PostgreSQL)
- 🔐 **Built-in authentication** (API Key based)
- 📝 **Schema-driven** table creation
- 🔧 **Modular architecture** with phase-based request processing
- ⚡ **High performance** with SO_REUSEPORT multi-threading

## Quick Start

### 1. Configure Database

Create `config/database.yaml`:

```yaml
datasource:
  sqlite1:
    driver: sqlite
    database: ./apify.sqlite
    max_pool_size: 5
  pg1:
    driver: postgres
    host: localhost
    port: 5432
    user: postgres
    password: postgres
    database: apify_db
    max_pool_size: 10
```

### 2. Define Your API

Create `config/openapi/users.yaml`:

```yaml
openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "Users API"
      version: "1.0.0"
    x-table-schemas:
      - table_name: "users"
        columns:
          - name: "id"
            column_type: "INTEGER"
            primary_key: true
            auto_increment: true
          - name: "name"
            column_type: "TEXT"
            nullable: false
          - name: "email"
            column_type: "TEXT"
            nullable: false
            unique: true
    paths:
      /users:
        get:
          operationId: listUsers
          x-modules:
            access: ["key_auth"]
          responses:
            "200": { description: "OK" }
        post:
          operationId: createUser
          x-modules:
            access: ["key_auth"]
          responses:
            "201": { description: "Created" }
      /users/{id}:
        get:
          operationId: getUser
          x-modules:
            access: ["key_auth"]
          responses:
            "200": { description: "OK" }
        put:
          operationId: updateUser
          x-modules:
            access: ["key_auth"]
          responses:
            "200": { description: "Updated" }
        delete:
          operationId: deleteUser
          x-modules:
            access: ["key_auth"]
          responses:
            "200": { description: "Deleted" }
datasource: sqlite1  # Specify which datasource to use
```

### 3. Configure Listeners

Create `config/config.yaml`:

```yaml
listeners:
  - port: 3000
    ip: 0.0.0.0
    protocol: HTTP
    apis:
      - path: ./openapi/users.yaml
      - path: ./openapi/books.yaml
    consumers:
      - name: default
        keys:
          - dev-key-123
```

### 4. Run

```bash
cargo run --package apify -- -c config/config.yaml
```

### 5. Test

```bash
# List users (requires authentication)
curl -H "X-Api-Key: dev-key-123" http://localhost:3000/users

# Create a user
curl -X POST -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{"name":"Alice","email":"alice@example.com"}' \
  http://localhost:3000/users

# Get a specific user
curl -H "X-Api-Key: dev-key-123" http://localhost:3000/users/1

# Update a user
curl -X PUT -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{"name":"Alice Smith","email":"alice.smith@example.com"}' \
  http://localhost:3000/users/1

# Delete a user
curl -X DELETE -H "X-Api-Key: dev-key-123" \
  http://localhost:3000/users/1
```

## Architecture

### Request Processing Phases

1. **HeaderParse** - Parse HTTP headers
2. **BodyParse** - Parse request body
3. **Rewrite** - URL rewriting (optional)
4. **Route** - Match request to operation
5. **Access** - Authentication & authorization
6. **Data** - Execute CRUD operations
7. **Response** - Return result

### Module System

Modules can be configured at three levels (higher priority wins):

1. **Operation-level** (in OpenAPI `x-modules`)
2. **Route-level** (per-API modules)
3. **Listener-level** (global fallback)

Example:
```yaml
# Operation-level module
paths:
  /users:
    get:
      x-modules:
        access: ["key_auth"]  # Require API key for this operation
```

## Multi-Database Configuration

Different APIs can use different databases:

```yaml
# config/database.yaml
datasource:
  main_db:
    driver: sqlite
    database: ./main.sqlite
  analytics_db:
    driver: postgres
    host: analytics.example.com
    port: 5432
    user: analytics
    password: secret
    database: analytics
```

```yaml
# config/openapi/users.yaml
datasource: main_db

# config/openapi/events.yaml
datasource: analytics_db
```

## Configuration Reference

### Database Configuration

```yaml
datasource:
  <name>:
    driver: sqlite | postgres
    database: <path-or-dbname>
    host: <hostname>          # postgres only
    port: <port>              # postgres only
    user: <username>          # postgres only
    password: <password>      # postgres only
    ssl_mode: <mode>          # postgres only
    max_pool_size: <number>   # optional, default 10
```

### OpenAPI Extensions

- `x-table-schemas`: Define database table schemas
- `x-modules`: Configure modules for operations/routes
  - `access`: Access control modules (e.g., `key_auth`)
  - `rewrite`: URL rewrite modules (future)

### Listeners

```yaml
listeners:
  - port: <number>
    ip: <ip-address>
    protocol: HTTP
    apis:
      - path: <openapi-file>
      - path: <openapi-file>
        modules:
          access: [<module-name>]
    consumers:
      - name: <consumer-name>
        keys: [<api-key>, ...]
    modules:
      access: [<module-name>]  # Listener-level fallback
```

## Development

### Build

```bash
cargo build --package apify
```

### Test

```bash
cargo test
```

### Run with custom threads

```bash
APIFY_THREADS=4 cargo run --package apify -- -c config/config.yaml
```

## License

[Add your license here]

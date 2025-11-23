# Apify

**Make everything as API** - A flexible, high-performance API framework that automatically generates CRUD operations from OpenAPI specifications.

[![Docker](https://img.shields.io/badge/docker-latest-blue)](https://hub.docker.com/r/apifyhost/apify)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

🚀 **Quick Start:** `curl -fsSL https://raw.githubusercontent.com/apifyhost/apify/main/quickstart.sh | bash`

[中文文档](./README.zh-CN.md) | [Getting Started](#-getting-started) | [API Usage](#-api-usage-guide) | [Configuration](#-configuration-guide)

---

### 🌟 Feature Highlights

#### 🚀 **Zero-Code CRUD Operations**
Define your data models in OpenAPI specs with `x-table-schemas`, and Apify automatically generates complete CRUD endpoints (Create, Read, Update, Delete) with database operations. No boilerplate code needed!

#### 🗄️ **Multi-Database Architecture**
- Support for **SQLite** and **PostgreSQL** backends
- Multiple datasources in one application
- Per-API database configuration
- Automatic connection pooling and management
- Schema auto-initialization from OpenAPI specs

#### 🔐 **Built-in Authentication & Audit Trail**
- **OpenAPI Security Scheme** support (standards-compliant)
- **API Key** authentication via `components.securitySchemes`
- **OAuth 2.0 / OpenID Connect** with OIDC discovery
  - Token introspection and JWT validation
  - Automatic JWKS caching
  - Issuer and audience validation
- **Automatic Audit Trail** - Track who created/modified records
  - `createdBy` / `updatedBy` fields auto-populated from OAuth identity
  - `createdAt` / `updatedAt` timestamps
  - Protection against user override attempts
- Consumer management with multiple keys
- Operation-level, route-level, and listener-level access control
- Extensible module system for custom auth methods

#### 🔧 **Modular Phase-Based Processing**
Request processing organized into 6 distinct phases:
1. **HeaderParse** - Extract and validate HTTP headers
2. **BodyParse** - Parse and validate request body (with validation modules)
3. **Route** - Match request to API operation
4. **Access** - Authentication and authorization
5. **Data** - Execute CRUD operations
6. **Response** - Format and return response (with header injection)
7. **Log** - Request and response logging

Each phase can have custom modules with flexible configuration at multiple levels.

#### ⚡ **High Performance**
- Multi-threaded architecture with **SO_REUSEPORT** socket sharing
- Configurable worker threads per listener
- Tokio async runtime for efficient I/O
- Zero-copy request routing where possible

#### 📝 **Schema-Driven Development**
- Define tables directly in OpenAPI specs
- Automatic DDL generation and execution
- Support for constraints, indexes, foreign keys
- Database schema versioning ready
- **Relations & Nested Objects** ✅ - Full CRUD support for related records
  - `hasMany` (one-to-many) - Parent with multiple children
  - `hasOne` (one-to-one) - Parent with single child
  - `belongsTo` (many-to-one) - Child references parent
  - Automatic foreign key injection
  - Nested data creation and retrieval
  - Auto-loading of relations in GET requests
  - Update nested relations (replace children)
  - Cascade delete for hasMany/hasOne

#### 🎯 **Flexible Configuration**
- YAML-based configuration
- Environment variable support
- Hot-reloadable API definitions (planned)
- Multiple listeners on different ports

---

### 🚀 Getting Started

#### Quickstart (Recommended)

The fastest way to get Apify running:

```bash
# Download and run the quickstart script
curl -fsSL https://raw.githubusercontent.com/apifyhost/apify/main/quickstart.sh | bash
```

Or download a specific release version:

```bash
# Download the quickstart package
curl -L https://github.com/apifyhost/apify/releases/download/v0.1.0/apify-quickstart-v0.1.0.tar.gz | tar xz
cd apify-quickstart-v0.1.0

# Make the script executable and run
chmod +x quickstart.sh
./quickstart.sh
```

The quickstart script will:
- ✅ Download and extract all necessary files
- ✅ Pull the Docker image
- ✅ Start Apify with SQLite
- ✅ Display access URLs and quick commands

**Quickstart Commands:**
```bash
./quickstart.sh install   # Download and install (default)
./quickstart.sh start     # Start services
./quickstart.sh stop      # Stop services
./quickstart.sh status    # Check service status
./quickstart.sh destroy   # Remove installation
```

#### Prerequisites

- **Rust** 1.70 or higher (for building from source)
- **Docker** (recommended for quick start)
- **SQLite** (included) or **PostgreSQL** server
- Basic knowledge of OpenAPI/Swagger

#### Manual Docker Setup

```bash
# Pull the latest image
docker pull apifyhost/apify:latest

# Run with SQLite
docker run -d \
  -p 3000:3000 \
  -v $(pwd)/config:/app/config:ro \
  -v apify-data:/app/data \
  apifyhost/apify:latest

# Or use Docker Compose
docker compose up -d
```

See [DOCKER.md](./DOCKER.md) for detailed Docker deployment guide.

#### Installation from Source

1. **Clone the repository**
   ```bash
   git clone https://github.com/apifyhost/apify.git
   cd apify
   ```

2. **Build the project**
   ```bash
   cargo build --release --package apify
   ```

3. **Run the binary**
   ```bash
   ./target/release/apify -c config.yaml
   ```

#### Quick Example: Building a User Management API

Let's build a complete user management API in 3 simple steps:

##### Step 1: Configure Your Main Config

Create `config/config.yaml`:

```yaml
# Global datasource configuration
datasource:
  sqlite1:
    driver: sqlite
    database: ./apify.sqlite
    max_pool_size: 5

# Global consumer (API key) configuration
consumers:
  - name: default
    keys:
      - dev-key-123
      - admin-key-456

# HTTP listeners
listeners:
  - port: 3000
    ip: 0.0.0.0
    protocol: HTTP
    apis:
      - path: openapi/users.yaml
        datasource: sqlite1  # Link API to datasource
        modules:
          access: ["key_auth"]  # Require authentication
```

##### Step 2: Define Your API with Schema

Create `config/openapi/users.yaml`:

```yaml
openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "Users API"
      version: "1.0.0"
    
    # Define authentication using OpenAPI security schemes
    components:
      securitySchemes:
        ApiKeyAuth:
          type: apiKey
          in: header
          name: X-Api-Key
    
    # Apply security globally (can be overridden per operation)
    security:
      - ApiKeyAuth: []
    
    # Define database table schema
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
          - name: "created_at"
            column_type: "TIMESTAMP"
            default: "CURRENT_TIMESTAMP"
    
    # Define API endpoints (CRUD auto-generated)
    paths:
      /users:
        get:
          operationId: listUsers
          summary: List all users
          responses:
            "200":
              description: List of users
        post:
          operationId: createUser
          summary: Create a new user
          responses:
            "201":
              description: User created
      
      /users/{id}:
        get:
          operationId: getUser
          summary: Get user by ID
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: integer
          responses:
            "200":
              description: User details
        put:
          operationId: updateUser
          summary: Update user
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: integer
          responses:
            "200":
              description: User updated
        delete:
          operationId: deleteUser
          summary: Delete user
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: integer
          responses:
            "204":
              description: User deleted
```

##### Step 3: Run and Test

Start the server:
```bash
cargo run --package apify -- -c config/config.yaml
```

Test your API:

```bash
# Create a user
curl -X POST http://localhost:3000/users \
  -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice Johnson",
    "email": "alice@example.com"
  }'

# Response: {"id": 1, "name": "Alice Johnson", "email": "alice@example.com", "created_at": "2024-11-09T10:30:00Z"}

# List all users
curl http://localhost:3000/users \
  -H "X-Api-Key: dev-key-123"

# Get specific user
curl http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123"

# Update user
curl -X PUT http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice Smith",
    "email": "alice.smith@example.com"
  }'

# Delete user
curl -X DELETE http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123"
```

That's it! You now have a fully functional CRUD API with authentication and database persistence.

---

### � API Usage Guide

Once your Apify server is running, you can interact with it using any HTTP client. Here's a comprehensive guide on how to call the APIs.

#### Authentication

Apify supports multiple authentication methods via OpenAPI security schemes:

##### API Key Authentication

When using API Key authentication (`key_auth` module), include the API key in the `X-Api-Key` header:

```bash
# Include the API key in every request
curl -H "X-Api-Key: your-api-key-here" http://localhost:3000/endpoint
```

Without authentication, you'll get a 401 Unauthorized response:
```bash
curl http://localhost:3000/users
# Response: 401 Unauthorized
```

##### OAuth 2.0 / OpenID Connect Authentication

When using OAuth/OIDC authentication (`oauth` module), include the bearer token in the `Authorization` header:

```bash
# First, obtain a token from your OAuth provider
TOKEN=$(curl -X POST https://your-oauth-provider.com/token \
  -d "grant_type=client_credentials" \
  -d "client_id=your-client-id" \
  -d "client_secret=your-secret" | jq -r .access_token)

# Use the token in API requests
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/users
```

Example with Keycloak:
```bash
# Get token using password grant (for testing)
TOKEN=$(curl -X POST http://localhost:8080/realms/apify/protocol/openid-connect/token \
  -d "grant_type=password" \
  -d "client_id=apify-client" \
  -d "client_secret=apify-secret" \
  -d "username=testuser" \
  -d "password=testpassword" | jq -r .access_token)

# Call protected endpoint
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/users
```

#### CRUD Operations

Based on your OpenAPI specification, Apify automatically generates the following operations:

##### **1. CREATE (POST)** - Add new records

```bash
# Create a single user
curl -X POST http://localhost:3000/users \
  -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice Johnson",
    "email": "alice@example.com"
  }'

# Response (201 Created):
{
  "id": 1,
  "name": "Alice Johnson",
  "email": "alice@example.com",
  "created_at": "2024-11-09T10:30:00Z"
}
```

##### **2. READ (GET)** - Retrieve records

**List all records:**
```bash
# Get all users
curl -H "X-Api-Key: dev-key-123" http://localhost:3000/users

# Response (200 OK):
[
  {
    "id": 1,
    "name": "Alice Johnson",
    "email": "alice@example.com",
    "created_at": "2024-11-09T10:30:00Z"
  },
  {
    "id": 2,
    "name": "Bob Smith",
    "email": "bob@example.com",
    "created_at": "2024-11-09T11:00:00Z"
  }
]
```

**Get single record by ID:**
```bash
# Get user with ID 1
curl -H "X-Api-Key: dev-key-123" http://localhost:3000/users/1

# Response (200 OK):
{
  "id": 1,
  "name": "Alice Johnson",
  "email": "alice@example.com",
  "created_at": "2024-11-09T10:30:00Z"
}

# If not found (404 Not Found):
{
  "error": "Record not found"
}
```

##### **3. UPDATE (PUT)** - Modify existing records

```bash
# Update user with ID 1
curl -X PUT http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice Smith",
    "email": "alice.smith@example.com"
  }'

# Response (200 OK):
{
  "id": 1,
  "name": "Alice Smith",
  "email": "alice.smith@example.com",
  "created_at": "2024-11-09T10:30:00Z"
}
```

##### **4. DELETE (DELETE)** - Remove records

```bash
# Delete user with ID 1
curl -X DELETE http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123"

# Response (204 No Content)
# Empty body, but successful deletion

# Trying to get deleted user (404 Not Found):
curl -H "X-Api-Key: dev-key-123" http://localhost:3000/users/1
```

#### Query Parameters (Future Support)

While basic CRUD is available now, advanced query features are planned:

```bash
# Filter records (planned)
curl -H "X-Api-Key: dev-key-123" \
  "http://localhost:3000/users?email=alice@example.com"

# Pagination (planned)
curl -H "X-Api-Key: dev-key-123" \
  "http://localhost:3000/users?page=1&limit=10"

# Sorting (planned)
curl -H "X-Api-Key: dev-key-123" \
  "http://localhost:3000/users?sort=-created_at"
```

#### Using Different HTTP Clients

**With JavaScript (fetch):**
```javascript
// Create user
const response = await fetch('http://localhost:3000/users', {
  method: 'POST',
  headers: {
    'X-Api-Key': 'dev-key-123',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    name: 'Alice Johnson',
    email: 'alice@example.com'
  })
});
const user = await response.json();
console.log(user);

// Get all users
const users = await fetch('http://localhost:3000/users', {
  headers: { 'X-Api-Key': 'dev-key-123' }
}).then(r => r.json());
```

**With Python (requests):**
```python
import requests

API_KEY = 'dev-key-123'
BASE_URL = 'http://localhost:3000'
headers = {'X-Api-Key': API_KEY}

# Create user
response = requests.post(
    f'{BASE_URL}/users',
    headers={**headers, 'Content-Type': 'application/json'},
    json={'name': 'Alice Johnson', 'email': 'alice@example.com'}
)
user = response.json()
print(user)

# Get all users
users = requests.get(f'{BASE_URL}/users', headers=headers).json()
print(users)

# Update user
updated = requests.put(
    f'{BASE_URL}/users/1',
    headers={**headers, 'Content-Type': 'application/json'},
    json={'name': 'Alice Smith', 'email': 'alice.smith@example.com'}
).json()

# Delete user
requests.delete(f'{BASE_URL}/users/1', headers=headers)
```

**With HTTPie:**
```bash
# Create
http POST localhost:3000/users X-Api-Key:dev-key-123 \
  name="Alice Johnson" email="alice@example.com"

# Read
http GET localhost:3000/users X-Api-Key:dev-key-123
http GET localhost:3000/users/1 X-Api-Key:dev-key-123

# Update
http PUT localhost:3000/users/1 X-Api-Key:dev-key-123 \
  name="Alice Smith" email="alice.smith@example.com"

# Delete
http DELETE localhost:3000/users/1 X-Api-Key:dev-key-123
```

#### HTTP Status Codes

Apify returns standard HTTP status codes:

| Status Code | Meaning | When It Happens |
|-------------|---------|-----------------|
| 200 OK | Success | GET, PUT operations succeeded |
| 201 Created | Resource created | POST operation succeeded |
| 204 No Content | Success, no body | DELETE operation succeeded |
| 400 Bad Request | Invalid request | Malformed JSON, missing fields |
| 401 Unauthorized | Authentication failed | Missing or invalid API key |
| 404 Not Found | Resource not found | GET/PUT/DELETE non-existent ID |
| 500 Internal Server Error | Server error | Database error, server crash |

#### Error Response Format

```json
{
  "error": "Error message here",
  "details": "Additional context (optional)"
}
```

#### Content Type

- **Request:** `Content-Type: application/json` for POST/PUT
- **Response:** Always `application/json`

---

### �📚 Core Concepts

---

### 📚 Core Concepts

#### Request Processing Pipeline

Every HTTP request flows through 7 phases:

```
HTTP Request
    ↓
1. HeaderParse  → Extract headers (auth tokens, content-type, etc.)
    ↓
2. BodyParse    → Parse JSON/form data
    ↓
3. Rewrite      → Transform URLs (optional)
    ↓
4. Route        → Match to OpenAPI operation
    ↓
5. Access       → Verify authentication/authorization
    ↓
6. Data         → Execute CRUD on database
    ↓
7. Response     → Format and send response
    ↓
HTTP Response
```

Each phase can be customized with modules.

#### Module Priority System

Modules are configured at three levels with cascading priority:

```
Operation-level (highest priority)
    ↓
Route-level (per-API)
    ↓
Listener-level (global fallback)
```

**Example:**
```yaml
# Listener-level (applies to all APIs)
listeners:
  - port: 3000
    modules:
      access: ["key_auth"]  # Default auth for everything
    
    apis:
      # Route-level (applies to this API)
      - path: openapi/users.yaml
        modules:
          access: ["oauth2"]  # Override with OAuth2 for users API
```

```yaml
# openapi/users.yaml - Operation-level
paths:
  /users/public:
    get:
      x-modules:
        access: []  # No auth required for this specific endpoint
```

#### Multi-Database Support

Different APIs can connect to different databases:

```yaml
# config.yaml
datasource:
  user_db:
    driver: sqlite
    database: ./users.sqlite
  
  analytics_db:
    driver: postgres
    host: analytics.example.com
    port: 5432
    user: analyst
    password: secret
    database: metrics

listeners:
  - port: 3000
    apis:
      - path: openapi/users.yaml
        datasource: user_db        # Users stored in SQLite
      
      - path: openapi/events.yaml
        datasource: analytics_db   # Events stored in PostgreSQL
```

---

### ⚙️ Configuration Reference

#### Main Configuration (`config.yaml`)

```yaml
# Global datasource definitions
datasource:
  <datasource-name>:
    driver: sqlite | postgres
    
    # SQLite specific
    database: <file-path>
    
    # PostgreSQL specific
    host: <hostname>
    port: <port-number>
    user: <username>
    password: <password>
    database: <database-name>
    ssl_mode: disable | require | prefer
    
    # Common
    max_pool_size: <number>  # Default: 10

# Global API consumers (API keys)
consumers:
  - name: <consumer-name>
    keys:
      - <api-key-1>
      - <api-key-2>

# HTTP listeners
listeners:
  - port: <port-number>
    ip: <ip-address>         # Default: 0.0.0.0
    protocol: HTTP
    
    # APIs served by this listener
    apis:
      - path: <openapi-file>
        datasource: <datasource-name>  # Optional
        modules:                        # Optional
          access: [<module-name>]
    
    # Listener-level modules (fallback)
    modules:
      access: [<module-name>]
    
    # Listener-level consumers (override global)
    consumers:
      - name: <consumer-name>
        keys: [<api-key>]
```

#### OpenAPI Extensions

Apify extends standard OpenAPI 3.0 with custom fields:

**`x-table-schemas`** - Define database tables (root level)
```yaml
x-table-schemas:
  - table_name: "users"
    columns:
      - name: "id"
        column_type: "INTEGER"
        primary_key: true
        auto_increment: true
      - name: "email"
        column_type: "TEXT"
        nullable: false
        unique: true
      - name: "status"
        column_type: "TEXT"
        default: "'active'"
```

**`x-modules`** - Configure modules per operation
```yaml
paths:
  /admin/users:
    get:
      x-modules:
        access: ["key_auth", "admin_check"]  # Multiple modules
```

**Supported Column Types:**
- SQLite: `INTEGER`, `TEXT`, `REAL`, `BLOB`, `TIMESTAMP`
- PostgreSQL: `INTEGER`, `BIGINT`, `TEXT`, `VARCHAR(n)`, `REAL`, `DOUBLE PRECISION`, `BOOLEAN`, `TIMESTAMP`, `DATE`, `JSON`, `JSONB`

**Column Constraints:**
- `primary_key: true` - Primary key
- `auto_increment: true` - Auto-increment (SQLite: INTEGER PRIMARY KEY, PG: SERIAL)
- `nullable: false` - NOT NULL constraint
- `unique: true` - UNIQUE constraint
- `default: "value"` - Default value

---

### 🛠️ Development

#### Building from Source

```bash
# Clone repository
git clone https://github.com/apifyhost/apify.git
cd apify

# Build all packages
cargo build --release

# Build specific package
cargo build --release --package apify

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run --package apify -- -c config.yaml
```

#### Running Tests

```bash
# All tests
cargo test

# Specific test file
cargo test --test integration_crud_users

# With output
cargo test -- --nocapture
```

#### Environment Variables

- `APIFY_THREADS` - Number of worker threads per listener (default: 2)
- `RUST_LOG` - Log level (error, warn, info, debug, trace)

#### Project Structure

```
apify/
├── apify/           # Main server package
│   ├── src/
│   │   ├── main.rs          # Entry point
│   │   ├── config.rs        # Configuration parsing
│   │   ├── server.rs        # HTTP server
│   │   ├── handler.rs       # Request handler
│   │   ├── database.rs      # Database facade
│   │   ├── crud_handler.rs  # CRUD operations
│   │   └── modules/         # Plugin modules
│   │       ├── sqlite.rs
│   │       └── postgres.rs
│   └── config/      # Configuration files
│       ├── config.yaml
│       └── openapi/
├── sdk/             # SDK for plugins
├── flow/            # Flow engine (optional)
└── plugins/         # Extension plugins
```

---

### 📖 Advanced Usage

#### Available Modules

Apify includes several built-in modules for different phases:

##### Access Phase Modules

**`key_auth`** - API Key Authentication
```yaml
# config.yaml
consumers:
  - name: mobile_app
    keys: ["key-123", "key-456"]

# In OpenAPI spec
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-Api-Key

security:
  - ApiKeyAuth: []
```

Example request:
```bash
curl -H "X-Api-Key: key-123" http://localhost:3000/users
```

**`oauth`** - OAuth 2.0 / OpenID Connect Authentication

Validates OAuth 2.0 bearer tokens using OIDC discovery and dual-path validation (introspection + JWT).

```yaml
# config.yaml
oauth_providers:
  - name: keycloak
    # Supports env var expansion: ${VAR:default}
    issuer: "${KEYCLOAK_URL:http://localhost:8080}/realms/${KEYCLOAK_REALM:apify}"
    client_id: "${KEYCLOAK_CLIENT_ID:apify-client}"
    client_secret: "${KEYCLOAK_CLIENT_SECRET}"
    audience: "apify-api"
    use_introspection: true

# In OpenAPI spec
components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
    # Or use OpenID Connect:
    OpenID:
      type: openIdConnect
      openIdConnectUrl: "http://localhost:8080/realms/apify/.well-known/openid-configuration"

security:
  - BearerAuth: []
```

Example request:
```bash
# Get token from OAuth provider (e.g., Keycloak)
TOKEN=$(curl -X POST http://localhost:8080/realms/apify/protocol/openid-connect/token \
  -d "grant_type=password" \
  -d "client_id=apify-client" \
  -d "client_secret=client-secret" \
  -d "username=testuser" \
  -d "password=testpass" | jq -r .access_token)

# Use token in API request
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/users
```

Features:
- Automatic OIDC discovery (`.well-known/openid-configuration`)
- Token introspection via OAuth provider
- Local JWT validation with JWKS
- Issuer and audience validation
- JWKS caching for performance

##### BodyParse Phase Modules

**`body_validator`** - Request Body Validation
Validates request body size and content-type headers.

```rust
// Usage example (in code)
use apify::modules::body_validator::{BodyValidator, BodyValidatorConfig};

let validator = BodyValidator::new(BodyValidatorConfig {
    max_body_size: 1024 * 1024, // 1MB limit
    enforce_content_type: true,
});
```

Features:
- Maximum body size enforcement
- Content-Type header validation for JSON
- Returns 413 Payload Too Large or 415 Unsupported Media Type

##### Response Phase Modules

**`response_headers`** - Custom Response Headers
Adds custom headers to all responses.

```rust
// Usage example (in code)
use apify::modules::response_headers::ResponseHeaders;

let module = ResponseHeaders::with_headers(vec![
    ("X-API-Version".to_string(), "v1".to_string()),
    ("X-Powered-By".to_string(), "Apify".to_string()),
]);
```

##### Log Phase Modules

**`request_logger`** - Request/Response Logging
Logs detailed information about requests and responses.

```rust
// Usage example (in code)
use apify::modules::request_logger::{RequestLogger, RequestLoggerConfig};

// Default configuration
let logger = RequestLogger::with_defaults();

// Verbose logging (includes body)
let logger = RequestLogger::verbose();

// Custom configuration
let logger = RequestLogger::new(RequestLoggerConfig {
    log_headers: true,
    log_body: false,      // Don't log body for security
    log_response: true,
});
```

Output example:
```
[1699564800123] GET /users/123 - matched_route: Some("/users/{id}")
  Query params: {"include": "profile"}
  Path params: {"id": "123"}
  Response: {"id":123,"name":"John Doe"}
```

#### Custom Authentication Module

You can override the `key_auth` module or create custom modules:

```yaml
# In operation x-modules
x-modules:
  access: ["custom_auth"]  # Your custom module
```

#### Multiple Listeners

Run different APIs on different ports:

```yaml
listeners:
  - port: 3000  # Public API
    apis:
      - path: openapi/public.yaml
  
  - port: 3001  # Admin API
    apis:
      - path: openapi/admin.yaml
    consumers:
      - name: admin
        keys: ["super-secret-key"]
```

#### Performance Tuning

**From Source:**
```bash
# Increase worker threads
APIFY_THREADS=8 ./apify -c config.yaml

# Adjust database pool size
datasource:
  main:
    max_pool_size: 50  # More connections
```

**With Docker:**
```bash
docker run -d \
  -e APIFY_THREADS=8 \
  -e RUST_LOG=info \
  ghcr.io/apifyhost/apify:latest
```

#### Docker Deployment

**Using Docker Compose (Recommended):**

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: apify
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: apify
    volumes:
      - postgres_data:/var/lib/postgresql/data

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

volumes:
  postgres_data:
```

**Building Custom Image:**

```bash
# Build
docker build -t apify:custom .

# Run
docker run -d \
  -p 3000:3000 \
  -v $(pwd)/config:/app/config:ro \
  apify:custom
```

See [DOCKER.md](./DOCKER.md) for comprehensive Docker documentation.

---

### 🧪 Testing

#### Unit and Integration Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test --test integration_modules

# Run with output
cargo test -- --nocapture
```

#### E2E Tests

The E2E test suite is written in Go using the Ginkgo BDD framework for better maintainability and readability.

```bash
# Quick start with Docker
./scripts/local-docker-test.sh

# Run tests manually
cd e2e
make deps      # Install dependencies
make test      # Run tests

# Or use docker compose
docker compose up -d
cd e2e && go test -v
```

The E2E test suite validates:
- ✅ SQLite database operations
- ✅ PostgreSQL database operations  
- ✅ CRUD operations (Create, Read, Update, Delete)
- ✅ Authentication and authorization
- ✅ API key validation
- ✅ OAuth 2.0 / OIDC authentication
- ✅ Relations (hasMany, hasOne, belongsTo)
- ✅ Nested data creation and auto-loading
- ✅ CASCADE DELETE operations
- ✅ Audit trail propagation
- ✅ Observability (metrics and tracing)
- ✅ Error handling
- ✅ Large payload handling
- ✅ Content-Type validation

See [e2e/README.md](./e2e/README.md) for detailed testing documentation.

---

### 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

### 📄 License

[Add your license here]

---

### 🔗 Resources

- [OpenAPI 3.0 Specification](https://swagger.io/specification/)
- [Rust Documentation](https://www.rust-lang.org/learn)
- [SQLite Documentation](https://www.sqlite.org/docs.html)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)

---


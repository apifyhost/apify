# Apify

**Make everything as API** - A flexible, high-performance API framework that automatically generates CRUD operations from OpenAPI specifications.

[English](#english) | [中文](#中文)

---

## English

### 🌟 Feature Highlights

#### 🚀 **Zero-Code CRUD Operations**
Define your data models in OpenAPI specs with `x-table-schemas`, and Apify automatically generates complete CRUD endpoints (Create, Read, Update, Delete) with database operations. No boilerplate code needed!

#### 🗄️ **Multi-Database Architecture**
- Support for **SQLite** and **PostgreSQL** backends
- Multiple datasources in one application
- Per-API database configuration
- Automatic connection pooling and management
- Schema auto-initialization from OpenAPI specs

#### 🔐 **Built-in Authentication**
- API Key-based authentication (`key_auth` module)
- Consumer management with multiple keys
- Operation-level, route-level, and listener-level access control
- Extensible module system for custom auth methods

#### 🔧 **Modular Phase-Based Processing**
Request processing organized into 7 distinct phases:
1. **HeaderParse** - Extract and validate HTTP headers
2. **BodyParse** - Parse and validate request body
3. **Rewrite** - URL rewriting and transformation
4. **Route** - Match request to API operation
5. **Access** - Authentication and authorization
6. **Data** - Execute CRUD operations
7. **Response** - Format and return response

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

#### 🎯 **Flexible Configuration**
- YAML-based configuration
- Environment variable support
- Hot-reloadable API definitions (planned)
- Multiple listeners on different ports

#### 🎯 **Flexible Configuration**
- YAML-based configuration
- Environment variable support
- Hot-reloadable API definitions (planned)
- Multiple listeners on different ports

---

### 🚀 Getting Started

#### Prerequisites

- **Rust** 1.70 or higher
- **SQLite** (included) or **PostgreSQL** server
- Basic knowledge of OpenAPI/Swagger

#### Installation

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

##### Step 2: Define Your API with Schema

Create `config/openapi/users.yaml`:

```yaml
openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "Users API"
      version: "1.0.0"
    
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

Apify uses **API Key authentication** via the `X-Api-Key` header (when `key_auth` module is enabled):

```bash
# Include the API key in every request
curl -H "X-Api-Key: your-api-key-here" http://localhost:3000/endpoint
```

Without authentication, you'll get a 401 Unauthorized response:
```bash
curl http://localhost:3000/users
# Response: 401 Unauthorized
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

```bash
# Increase worker threads
APIFY_THREADS=8 ./apify -c config.yaml

# Adjust database pool size
datasource:
  main:
    max_pool_size: 50  # More connections
```

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

## 中文

### 🌟 功能特性

#### 🚀 **零代码 CRUD 操作**
在 OpenAPI 规范中通过 `x-table-schemas` 定义数据模型，Apify 自动生成完整的 CRUD 端点（创建、读取、更新、删除）及数据库操作。无需编写样板代码！

#### 🗄️ **多数据库架构**
- 支持 **SQLite** 和 **PostgreSQL** 后端
- 单个应用程序支持多个数据源
- 每个 API 可配置独立数据库
- 自动连接池管理
- 从 OpenAPI 规范自动初始化数据库模式

#### 🔐 **内置认证**
- 基于 API Key 的认证（`key_auth` 模块）
- 支持多密钥的消费者管理
- 操作级、路由级、监听器级访问控制
- 可扩展的模块系统支持自定义认证方法

#### 🔧 **模块化阶段处理**
请求处理分为 7 个独立阶段：
1. **HeaderParse** - 提取和验证 HTTP 头
2. **BodyParse** - 解析和验证请求体
3. **Rewrite** - URL 重写和转换
4. **Route** - 匹配请求到 API 操作
5. **Access** - 认证和授权
6. **Data** - 执行 CRUD 操作
7. **Response** - 格式化和返回响应

每个阶段都可以配置自定义模块，支持多级灵活配置。

#### ⚡ **高性能**
- 基于 **SO_REUSEPORT** 套接字共享的多线程架构
- 每个监听器可配置工作线程数
- Tokio 异步运行时实现高效 I/O
- 尽可能使用零拷贝请求路由

#### 📝 **模式驱动开发**
- 直接在 OpenAPI 规范中定义表结构
- 自动生成和执行 DDL
- 支持约束、索引、外键
- 数据库模式版本控制（规划中）

#### 🎯 **灵活配置**
- 基于 YAML 的配置
- 支持环境变量
- 热重载 API 定义（规划中）
- 不同端口上运行多个监听器

---

### 🚀 快速开始

#### 前置要求

- **Rust** 1.70 或更高版本
- **SQLite**（内置）或 **PostgreSQL** 服务器
- OpenAPI/Swagger 基础知识

#### 安装

1. **克隆仓库**
   ```bash
   git clone https://github.com/apifyhost/apify.git
   cd apify
   ```

2. **构建项目**
   ```bash
   cargo build --release --package apify
   ```

3. **运行程序**
   ```bash
   ./target/release/apify -c config.yaml
   ```

#### 快速示例：构建用户管理 API

通过 3 个简单步骤构建完整的用户管理 API：

##### 步骤 1：配置主配置文件

创建 `config/config.yaml`：

```yaml
# 全局数据源配置
datasource:
  sqlite1:
    driver: sqlite
    database: ./apify.sqlite
    max_pool_size: 5

# 全局消费者（API 密钥）配置
consumers:
  - name: default
    keys:
      - dev-key-123
      - admin-key-456

# HTTP 监听器
listeners:
  - port: 3000
    ip: 0.0.0.0
    protocol: HTTP
    apis:
      - path: openapi/users.yaml
        datasource: sqlite1  # 将 API 链接到数据源
        modules:
          access: ["key_auth"]  # 要求认证
```

##### 步骤 2：定义带模式的 API

创建 `config/openapi/users.yaml`：

```yaml
openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "用户 API"
      version: "1.0.0"
    
    # 定义数据库表模式
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
    
    # 定义 API 端点（CRUD 自动生成）
    paths:
      /users:
        get:
          operationId: listUsers
          summary: 列出所有用户
          responses:
            "200":
              description: 用户列表
        post:
          operationId: createUser
          summary: 创建新用户
          responses:
            "201":
              description: 用户已创建
      
      /users/{id}:
        get:
          operationId: getUser
          summary: 根据 ID 获取用户
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: integer
          responses:
            "200":
              description: 用户详情
        put:
          operationId: updateUser
          summary: 更新用户
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: integer
          responses:
            "200":
              description: 用户已更新
        delete:
          operationId: deleteUser
          summary: 删除用户
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: integer
          responses:
            "204":
              description: 用户已删除
```

##### 步骤 3：运行和测试

启动服务器：
```bash
cargo run --package apify -- -c config/config.yaml
```

测试 API：

```bash
# 创建用户
curl -X POST http://localhost:3000/users \
  -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "张三",
    "email": "zhangsan@example.com"
  }'

# 响应: {"id": 1, "name": "张三", "email": "zhangsan@example.com", "created_at": "2024-11-09T10:30:00Z"}

# 列出所有用户
curl http://localhost:3000/users \
  -H "X-Api-Key: dev-key-123"

# 获取特定用户
curl http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123"

# 更新用户
curl -X PUT http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "张三丰",
    "email": "zhangsanfeng@example.com"
  }'

# 删除用户
curl -X DELETE http://localhost:3000/users/1 \
  -H "X-Api-Key: dev-key-123"
```

完成！现在您已经拥有一个功能齐全的 CRUD API，包含认证和数据库持久化。

---

### 📚 核心概念

#### 请求处理管道

每个 HTTP 请求都经过 7 个阶段：

```
HTTP 请求
    ↓
1. HeaderParse  → 提取请求头（认证令牌、内容类型等）
    ↓
2. BodyParse    → 解析 JSON/表单数据
    ↓
3. Rewrite      → 转换 URL（可选）
    ↓
4. Route        → 匹配到 OpenAPI 操作
    ↓
5. Access       → 验证认证/授权
    ↓
6. Data         → 在数据库上执行 CRUD
    ↓
7. Response     → 格式化并发送响应
    ↓
HTTP 响应
```

每个阶段都可以通过模块自定义。

#### 模块优先级系统

模块可在三个级别配置，优先级递减：

```
操作级（最高优先级）
    ↓
路由级（每个 API）
    ↓
监听器级（全局后备）
```

**示例：**
```yaml
# 监听器级（应用于所有 API）
listeners:
  - port: 3000
    modules:
      access: ["key_auth"]  # 所有内容的默认认证
    
    apis:
      # 路由级（应用于此 API）
      - path: openapi/users.yaml
        modules:
          access: ["oauth2"]  # 为用户 API 覆盖为 OAuth2
```

```yaml
# openapi/users.yaml - 操作级
paths:
  /users/public:
    get:
      x-modules:
        access: []  # 此特定端点不需要认证
```

#### 多数据库支持

不同的 API 可以连接到不同的数据库：

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
        datasource: user_db        # 用户存储在 SQLite 中
      
      - path: openapi/events.yaml
        datasource: analytics_db   # 事件存储在 PostgreSQL 中
```

---

### ⚙️ 配置参考

#### 主配置文件 (`config.yaml`)

```yaml
# 全局数据源定义
datasource:
  <数据源名称>:
    driver: sqlite | postgres
    
    # SQLite 特定配置
    database: <文件路径>
    
    # PostgreSQL 特定配置
    host: <主机名>
    port: <端口号>
    user: <用户名>
    password: <密码>
    database: <数据库名>
    ssl_mode: disable | require | prefer
    
    # 通用配置
    max_pool_size: <数字>  # 默认: 10

# 全局 API 消费者（API 密钥）
consumers:
  - name: <消费者名称>
    keys:
      - <api密钥-1>
      - <api密钥-2>

# HTTP 监听器
listeners:
  - port: <端口号>
    ip: <IP地址>         # 默认: 0.0.0.0
    protocol: HTTP
    
    # 此监听器提供的 API
    apis:
      - path: <openapi文件>
        datasource: <数据源名称>  # 可选
        modules:                  # 可选
          access: [<模块名>]
    
    # 监听器级模块（后备）
    modules:
      access: [<模块名>]
    
    # 监听器级消费者（覆盖全局）
    consumers:
      - name: <消费者名称>
        keys: [<api密钥>]
```

#### OpenAPI 扩展

Apify 通过自定义字段扩展标准 OpenAPI 3.0：

**`x-table-schemas`** - 定义数据库表（根级别）
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

**`x-modules`** - 为每个操作配置模块
```yaml
paths:
  /admin/users:
    get:
      x-modules:
        access: ["key_auth", "admin_check"]  # 多个模块
```

**支持的列类型：**
- SQLite: `INTEGER`, `TEXT`, `REAL`, `BLOB`, `TIMESTAMP`
- PostgreSQL: `INTEGER`, `BIGINT`, `TEXT`, `VARCHAR(n)`, `REAL`, `DOUBLE PRECISION`, `BOOLEAN`, `TIMESTAMP`, `DATE`, `JSON`, `JSONB`

**列约束：**
- `primary_key: true` - 主键
- `auto_increment: true` - 自增（SQLite: INTEGER PRIMARY KEY，PG: SERIAL）
- `nullable: false` - NOT NULL 约束
- `unique: true` - UNIQUE 约束
- `default: "value"` - 默认值

---

### 🛠️ 开发

#### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/apifyhost/apify.git
cd apify

# 构建所有包
cargo build --release

# 构建特定包
cargo build --release --package apify

# 运行测试
cargo test

# 使用调试日志运行
RUST_LOG=debug cargo run --package apify -- -c config.yaml
```

#### 运行测试

```bash
# 所有测试
cargo test

# 特定测试文件
cargo test --test integration_crud_users

# 显示输出
cargo test -- --nocapture
```

#### 环境变量

- `APIFY_THREADS` - 每个监听器的工作线程数（默认: 2）
- `RUST_LOG` - 日志级别（error、warn、info、debug、trace）

#### 项目结构

```
apify/
├── apify/           # 主服务器包
│   ├── src/
│   │   ├── main.rs          # 入口点
│   │   ├── config.rs        # 配置解析
│   │   ├── server.rs        # HTTP 服务器
│   │   ├── handler.rs       # 请求处理器
│   │   ├── database.rs      # 数据库门面
│   │   ├── crud_handler.rs  # CRUD 操作
│   │   └── modules/         # 插件模块
│   │       ├── sqlite.rs
│   │       └── postgres.rs
│   └── config/      # 配置文件
│       ├── config.yaml
│       └── openapi/
├── sdk/             # 插件 SDK
├── flow/            # 流引擎（可选）
└── plugins/         # 扩展插件
```

---

### 📖 高级用法

#### 自定义认证模块

您可以覆盖 `key_auth` 模块或创建自定义模块：

```yaml
# 在操作 x-modules 中
x-modules:
  access: ["custom_auth"]  # 您的自定义模块
```

#### 多个监听器

在不同端口上运行不同的 API：

```yaml
listeners:
  - port: 3000  # 公共 API
    apis:
      - path: openapi/public.yaml
  
  - port: 3001  # 管理 API
    apis:
      - path: openapi/admin.yaml
    consumers:
      - name: admin
        keys: ["super-secret-key"]
```

#### 性能调优

```bash
# 增加工作线程
APIFY_THREADS=8 ./apify -c config.yaml

# 调整数据库连接池大小
datasource:
  main:
    max_pool_size: 50  # 更多连接
```

---

### 🤝 贡献

欢迎贡献！请随时提交 Pull Request。

1. Fork 仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m '添加某个很棒的特性'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

---

### 📄 许可证

[在此添加您的许可证]

---

### 🔗 资源

- [OpenAPI 3.0 规范](https://swagger.io/specification/)
- [Rust 文档](https://www.rust-lang.org/zh-CN/learn)
- [SQLite 文档](https://www.sqlite.org/docs.html)
- [PostgreSQL 文档](https://www.postgresql.org/docs/)

---

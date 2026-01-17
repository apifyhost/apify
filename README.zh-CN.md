# Apify

**让一切皆为 API** - 一个灵活、高性能的 API 框架，可从 OpenAPI 规范自动生成 CRUD 操作。

[English](./README.md) | [中文](#)

---

## 🌟 功能特性

### 🚀 **零代码 CRUD 操作**
在 OpenAPI 规范中通过 `x-table-schemas` 定义数据模型，Apify 自动生成完整的 CRUD 端点（创建、读取、更新、删除）及数据库操作。无需编写样板代码！

### 🗄️ **多数据库架构**
- 支持 **SQLite** 和 **PostgreSQL** 后端
- 单个应用程序支持多个数据源
- 每个 API 可配置独立数据库
- 自动连接池管理
- 从 OpenAPI 规范自动初始化数据库模式

### 🔐 **内置认证**
- **OpenAPI Security Scheme** 支持（符合标准规范）
- 基于 API Key 的认证，通过 `components.securitySchemes` 定义
- 支持多密钥的消费者管理
- 操作级、路由级、监听器级访问控制
- 可扩展的模块系统支持自定义认证方法

### 🔧 **模块化阶段处理**
请求处理分为 7 个独立阶段：
1. **HeaderParse** - 提取和验证 HTTP 头
2. **BodyParse** - 解析和验证请求体（支持验证模块）
3. **Route** - 匹配请求到 API 操作
4. **Access** - 认证和授权
5. **Data** - 执行 CRUD 操作
6. **Response** - 格式化和返回响应（支持响应头注入）
7. **Log** - 请求和响应日志记录

每个阶段都可以配置自定义模块，支持多级灵活配置。

### ⚡ **高性能**
- 基于 **SO_REUSEPORT** 套接字共享的多线程架构
- 每个监听器可配置工作线程数
- Tokio 异步运行时实现高效 I/O
- 尽可能使用零拷贝请求路由

### 📝 **模式驱动开发**
- 直接在 OpenAPI 规范中定义表结构
- 自动生成和执行 DDL
- 支持约束、索引、外键
- 数据库模式版本控制（规划中）

### 🎯 **灵活配置**
- 基于 YAML 的配置
- 支持环境变量
- 热重载 API 定义（规划中）
- 不同端口上运行多个监听器

---

## 🚀 快速开始

### 前置要求

- **Rust** 1.70 或更高版本
- **SQLite**（内置）或 **PostgreSQL** 服务器
- OpenAPI/Swagger 基础知识

### 安装

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

### 快速示例：构建用户管理 API

通过 3 个简单步骤构建完整的用户管理 API：

#### 步骤 1：配置主配置文件

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

#### 步骤 2：定义带模式的 API

创建 `config/openapi/users.yaml`：

```yaml
openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "用户 API"
      version: "1.0.0"
    
    # 使用 OpenAPI 安全方案定义认证
    components:
      securitySchemes:
        ApiKeyAuth:
          type: apiKey
          in: header
          name: X-API-KEY
    
    # 全局应用安全策略（可在具体操作中覆盖）
    security:
      - ApiKeyAuth: []
    
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

#### 步骤 3：运行和测试

启动服务器：
```bash
cargo run --package apify -- -c config/config.yaml
```

测试 API：

```bash
# 创建用户
curl -X POST http://localhost:3000/users \
  -H "X-API-KEY: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "张三",
    "email": "zhangsan@example.com"
  }'

# 响应: {"id": 1, "name": "张三", "email": "zhangsan@example.com", "created_at": "2024-11-09T10:30:00Z"}

# 列出所有用户
curl http://localhost:3000/users \
  -H "X-API-KEY: dev-key-123"

# 获取特定用户
curl http://localhost:3000/users/1 \
  -H "X-API-KEY: dev-key-123"

# 更新用户
curl -X PUT http://localhost:3000/users/1 \
  -H "X-API-KEY: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "张三丰",
    "email": "zhangsanfeng@example.com"
  }'

# 删除用户
curl -X DELETE http://localhost:3000/users/1 \
  -H "X-API-KEY: dev-key-123"
```

完成！现在您已经拥有一个功能齐全的 CRUD API，包含认证和数据库持久化。

---

### 📡 API 调用指南

一旦您的 Apify 服务器运行起来，您可以使用任何 HTTP 客户端与它交互。以下是如何调用 API 的完整指南。

#### 认证方式

Apify 使用 **API Key 认证**，通过 `X-API-KEY` 请求头传递（当启用 `key_auth` 模块时）：

```bash
# 在每个请求中包含 API 密钥
curl -H "X-API-KEY: your-api-key-here" http://localhost:3000/endpoint
```

没有认证时，您会收到 401 未授权响应：
```bash
curl http://localhost:3000/users
# 响应: 401 Unauthorized
```

#### CRUD 操作

基于您的 OpenAPI 规范，Apify 自动生成以下操作：

##### **1. CREATE (POST)** - 创建新记录

```bash
# 创建单个用户
curl -X POST http://localhost:3000/users \
  -H "X-API-KEY: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "张三",
    "email": "zhangsan@example.com"
  }'

# 响应 (201 Created):
{
  "id": 1,
  "name": "张三",
  "email": "zhangsan@example.com",
  "created_at": "2024-11-09T10:30:00Z"
}
```

##### **2. READ (GET)** - 读取记录

**列出所有记录：**
```bash
# 获取所有用户
curl -H "X-API-KEY: dev-key-123" http://localhost:3000/users

# 响应 (200 OK):
[
  {
    "id": 1,
    "name": "张三",
    "email": "zhangsan@example.com",
    "created_at": "2024-11-09T10:30:00Z"
  },
  {
    "id": 2,
    "name": "李四",
    "email": "lisi@example.com",
    "created_at": "2024-11-09T11:00:00Z"
  }
]
```

**根据 ID 获取单条记录：**
```bash
# 获取 ID 为 1 的用户
curl -H "X-API-KEY: dev-key-123" http://localhost:3000/users/1

# 响应 (200 OK):
{
  "id": 1,
  "name": "张三",
  "email": "zhangsan@example.com",
  "created_at": "2024-11-09T10:30:00Z"
}

# 如果未找到 (404 Not Found):
{
  "error": "Record not found"
}
```

##### **3. UPDATE (PUT)** - 更新现有记录

```bash
# 更新 ID 为 1 的用户
curl -X PUT http://localhost:3000/users/1 \
  -H "X-API-KEY: dev-key-123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "张三丰",
    "email": "zhangsanfeng@example.com"
  }'

# 响应 (200 OK):
{
  "id": 1,
  "name": "张三丰",
  "email": "zhangsanfeng@example.com",
  "created_at": "2024-11-09T10:30:00Z"
}
```

##### **4. DELETE (DELETE)** - 删除记录

```bash
# 删除 ID 为 1 的用户
curl -X DELETE http://localhost:3000/users/1 \
  -H "X-API-KEY: dev-key-123"

# 响应 (204 No Content)
# 空响应体，但删除成功

# 尝试获取已删除的用户 (404 Not Found):
curl -H "X-API-KEY: dev-key-123" http://localhost:3000/users/1
```

#### 查询参数（未来支持）

虽然基本的 CRUD 操作现在可用，但高级查询功能正在规划中：

```bash
# 过滤记录（规划中）
curl -H "X-API-KEY: dev-key-123" \
  "http://localhost:3000/users?email=zhangsan@example.com"

# 分页（规划中）
curl -H "X-API-KEY: dev-key-123" \
  "http://localhost:3000/users?page=1&limit=10"

# 排序（规划中）
curl -H "X-API-KEY: dev-key-123" \
  "http://localhost:3000/users?sort=-created_at"
```

#### 使用不同的 HTTP 客户端

**使用 JavaScript (fetch):**
```javascript
// 创建用户
const response = await fetch('http://localhost:3000/users', {
  method: 'POST',
  headers: {
    'X-API-KEY': 'dev-key-123',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    name: '张三',
    email: 'zhangsan@example.com'
  })
});
const user = await response.json();
console.log(user);

// 获取所有用户
const users = await fetch('http://localhost:3000/users', {
  headers: { 'X-API-KEY': 'dev-key-123' }
}).then(r => r.json());
```

**使用 Python (requests):**
```python
import requests

API_KEY = 'dev-key-123'
BASE_URL = 'http://localhost:3000'
headers = {'X-API-KEY': API_KEY}

# 创建用户
response = requests.post(
    f'{BASE_URL}/users',
    headers={**headers, 'Content-Type': 'application/json'},
    json={'name': '张三', 'email': 'zhangsan@example.com'}
)
user = response.json()
print(user)

# 获取所有用户
users = requests.get(f'{BASE_URL}/users', headers=headers).json()
print(users)

# 更新用户
updated = requests.put(
    f'{BASE_URL}/users/1',
    headers={**headers, 'Content-Type': 'application/json'},
    json={'name': '张三丰', 'email': 'zhangsanfeng@example.com'}
).json()

# 删除用户
requests.delete(f'{BASE_URL}/users/1', headers=headers)
```

**使用 HTTPie:**
```bash
# 创建
http POST localhost:3000/users X-API-KEY:dev-key-123 \
  name="张三" email="zhangsan@example.com"

# 读取
http GET localhost:3000/users X-API-KEY:dev-key-123
http GET localhost:3000/users/1 X-API-KEY:dev-key-123

# 更新
http PUT localhost:3000/users/1 X-API-KEY:dev-key-123 \
  name="张三丰" email="zhangsanfeng@example.com"

# 删除
http DELETE localhost:3000/users/1 X-API-KEY:dev-key-123
```

#### HTTP 状态码

Apify 返回标准的 HTTP 状态码：

| 状态码 | 含义 | 发生时机 |
|--------|------|----------|
| 200 OK | 成功 | GET、PUT 操作成功 |
| 201 Created | 资源已创建 | POST 操作成功 |
| 204 No Content | 成功，无响应体 | DELETE 操作成功 |
| 400 Bad Request | 无效请求 | JSON 格式错误、缺少字段 |
| 401 Unauthorized | 认证失败 | 缺少或无效的 API 密钥 |
| 404 Not Found | 资源未找到 | GET/PUT/DELETE 不存在的 ID |
| 500 Internal Server Error | 服务器错误 | 数据库错误、服务器崩溃 |

#### 错误响应格式

```json
{
  "error": "错误消息",
  "details": "附加上下文（可选）"
}
```

#### 内容类型

- **请求：** POST/PUT 使用 `Content-Type: application/json`
- **响应：** 始终为 `application/json`

---

## 📚 核心概念

### 请求处理管道

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

### 模块优先级系统

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

### 多数据库支持

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

## ⚙️ 配置参考

### 主配置文件 (`config.yaml`)

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

### OpenAPI 扩展

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

## 🛠️ 开发

### 从源码构建

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

### 运行测试

```bash
# 所有测试
cargo test

# 特定测试文件
cargo test --test integration_crud_users

# 显示输出
cargo test -- --nocapture
```

### 环境变量

- `APIFY_THREADS` - 每个监听器的工作线程数（默认: 2）
- `RUST_LOG` - 日志级别（error、warn、info、debug、trace）

### 项目结构

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

## 📖 高级用法

### 可用模块

Apify 包含多个内置模块用于不同阶段：

#### Access 阶段模块

**`key_auth`** - API Key 认证
```yaml
# config.yaml
consumers:
  - name: mobile_app
    keys: ["key-123", "key-456"]

# 在 OpenAPI 规范中
x-modules:
  access: ["key_auth"]
```

示例请求：
```bash
curl -H "X-API-KEY: key-123" http://localhost:3000/users
```

#### BodyParse 阶段模块

**`request_validator`** - 请求验证 (Body, Query, Headers)
验证请求体大小和 Content-Type 头。

```rust
// 使用示例（代码中）
use apify::modules::request_validator::{RequestValidator, RequestValidatorConfig};

let validator = BodyValidator::new(BodyValidatorConfig {
    max_body_size: 1024 * 1024, // 1MB 限制
    enforce_content_type: true,
});
```

功能：
- 强制执行最大请求体大小
- JSON 的 Content-Type 头验证
- 返回 413 Payload Too Large 或 415 Unsupported Media Type

#### Response 阶段模块

**`response_headers`** - 自定义响应头
为所有响应添加自定义头。

```rust
// 使用示例（代码中）
use apify::modules::response_headers::ResponseHeaders;

let module = ResponseHeaders::with_headers(vec![
    ("X-API-Version".to_string(), "v1".to_string()),
    ("X-Powered-By".to_string(), "Apify".to_string()),
]);
```

#### Log 阶段模块

**`request_logger`** - 请求/响应日志
记录请求和响应的详细信息。

```rust
// 使用示例（代码中）
use apify::modules::request_logger::{RequestLogger, RequestLoggerConfig};

// 默认配置
let logger = RequestLogger::with_defaults();

// 详细日志（包含请求体）
let logger = RequestLogger::verbose();

// 自定义配置
let logger = RequestLogger::new(RequestLoggerConfig {
    log_headers: true,
    log_body: false,      // 出于安全考虑不记录请求体
    log_response: true,
});
```

输出示例：
```
[1699564800123] GET /users/123 - matched_route: Some("/users/{id}")
  Query params: {"include": "profile"}
  Path params: {"id": "123"}
  Response: {"id":123,"name":"张三"}
```

### 自定义认证模块

您可以覆盖 `key_auth` 模块或创建自定义模块：

```yaml
# 在操作 x-modules 中
x-modules:
  access: ["custom_auth"]  # 您的自定义模块
```

### 多个监听器

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

### 性能调优

```bash
# 增加工作线程
APIFY_THREADS=8 ./apify -c config.yaml

# 调整数据库连接池大小
datasource:
  main:
    max_pool_size: 50  # 更多连接
```

---

## 🤝 贡献

欢迎贡献！请随时提交 Pull Request。

1. Fork 仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m '添加某个很棒的特性'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

---

## 📄 许可证

[在此添加您的许可证]

---

## 🔗 资源

- [OpenAPI 3.0 规范](https://swagger.io/specification/)
- [Rust 文档](https://www.rust-lang.org/zh-CN/learn)
- [SQLite 文档](https://www.sqlite.org/docs.html)
- [PostgreSQL 文档](https://www.postgresql.org/docs/)

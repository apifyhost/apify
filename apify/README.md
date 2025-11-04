# Apify - 无代码 API 服务

Apify 是一个基于 OpenAPI 规范自动生成数据库 CRUD API 的无代码服务。

## 功能特性

- 🚀 **自动 API 生成**: 根据 OpenAPI 规范自动生成 RESTful API
- 🗄️ **数据库 CRUD**: 支持 PostgreSQL 数据库的增删改查操作
- 📝 **OpenAPI 验证**: 内置请求和响应验证
- 🔧 **配置驱动**: 通过 YAML 配置文件轻松配置
- ⚡ **高性能**: 基于 Rust 和 Tokio 异步运行时

## 快速开始

### 1. 安装依赖

确保你的系统已安装：
- Rust (最新稳定版)
- PostgreSQL 数据库

### 2. 配置数据库

创建 PostgreSQL 数据库：

```sql
CREATE DATABASE apify_db;
CREATE USER apify_user WITH PASSWORD 'apify_password';
GRANT ALL PRIVILEGES ON DATABASE apify_db TO apify_user;
```

### 3. 创建示例表

```sql
-- 连接到 apify_db 数据库
\c apify_db;

-- 创建用户表
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 插入示例数据
INSERT INTO users (name, email) VALUES 
    ('张三', 'zhangsan@example.com'),
    ('李四', 'lisi@example.com');
```

### 4. 配置服务

使用提供的示例配置文件 `config/crud-config.yaml`：

```yaml
listeners:
  - port: 3000
    ip: 0.0.0.0
    protocol: HTTP
    routes:
    - name: api
      matches:
      - path:
          path_prefix: /
        method: GET

# 数据库配置
database:
  host: localhost
  port: 5432
  user: apify_user
  password: apify_password
  database: apify_db
  ssl_mode: prefer
  max_pool_size: 10

# OpenAPI 规范
openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "Apify CRUD API"
      version: "1.0.0"
      description: "Auto-generated CRUD API"
    paths:
      /users:
        get:
          summary: "获取所有用户"
          operationId: "listUsers"
          responses:
            "200":
              description: "用户列表"
              content:
                application/json:
                  schema:
                    type: array
                    items:
                      $ref: "#/components/schemas/User"
        post:
          summary: "创建新用户"
          operationId: "createUser"
          requestBody:
            required: true
            content:
              application/json:
                schema:
                  $ref: "#/components/schemas/NewUser"
          responses:
            "201":
              description: "用户创建成功"
              content:
                application/json:
                  schema:
                    $ref: "#/components/schemas/User"
      /users/{id}:
        get:
          summary: "根据ID获取用户"
          operationId: "getUser"
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: string
          responses:
            "200":
              description: "用户信息"
              content:
                application/json:
                  schema:
                    $ref: "#/components/schemas/User"
            "404":
              description: "用户不存在"
        put:
          summary: "更新用户"
          operationId: "updateUser"
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: string
          requestBody:
            required: true
            content:
              application/json:
                schema:
                  $ref: "#/components/schemas/UserUpdate"
          responses:
            "200":
              description: "用户更新成功"
              content:
                application/json:
                  schema:
                    $ref: "#/components/schemas/User"
        delete:
          summary: "删除用户"
          operationId: "deleteUser"
          parameters:
            - name: id
              in: path
              required: true
              schema:
                type: string
          responses:
            "200":
              description: "用户删除成功"
              content:
                application/json:
                  schema:
                    type: object
                    properties:
                      message:
                        type: string
                      affected_rows:
                        type: integer
    components:
      schemas:
        User:
          type: object
          properties:
            id:
              type: string
            name:
              type: string
            email:
              type: string
            created_at:
              type: string
              format: date-time
        NewUser:
          type: object
          required:
            - name
            - email
          properties:
            name:
              type: string
              minLength: 1
            email:
              type: string
              format: email
        UserUpdate:
          type: object
          properties:
            name:
              type: string
              minLength: 1
            email:
              type: string
              format: email
  validation:
    strict_mode: true
    validate_request_body: true
    validate_response_body: false
```

### 5. 启动服务

```bash
cd apify
cargo run -- --config config/crud-config.yaml
```

### 6. 测试 API

服务启动后，你可以使用以下命令测试 API：

#### 获取所有用户
```bash
curl http://localhost:3000/users
```

#### 根据ID获取用户
```bash
curl http://localhost:3000/users/1
```

#### 创建新用户
```bash
curl -X POST http://localhost:3000/users \
  -H "Content-Type: application/json" \
  -d '{"name": "王五", "email": "wangwu@example.com"}'
```

#### 更新用户
```bash
curl -X PUT http://localhost:3000/users/1 \
  -H "Content-Type: application/json" \
  -d '{"name": "张三（更新）", "email": "zhangsan_new@example.com"}'
```

#### 删除用户
```bash
curl -X DELETE http://localhost:3000/users/1
```

## API 特性

### 自动 CRUD 操作

Apify 根据 OpenAPI 规范中的路径和方法自动生成以下操作：

- `GET /table` - 获取所有记录（支持分页和过滤）
- `GET /table/{id}` - 根据ID获取单条记录
- `POST /table` - 创建新记录
- `PUT /table/{id}` - 更新记录
- `DELETE /table/{id}` - 删除记录

### 查询参数支持

GET 请求支持以下查询参数：

- `limit` - 限制返回记录数
- `offset` - 跳过记录数（分页）
- 其他字段名 - 用于过滤（WHERE 条件）

示例：
```bash
# 分页查询
curl "http://localhost:3000/users?limit=10&offset=20"

# 过滤查询
curl "http://localhost:3000/users?name=张三"
```

### 数据验证

- 请求体验证：根据 OpenAPI schema 验证请求体
- 字段验证：支持 minLength、maxLength、pattern 等验证规则
- 类型验证：自动验证字段类型

## 配置说明

### 数据库配置

```yaml
database:
  host: localhost          # 数据库主机
  port: 5432              # 数据库端口
  user: username          # 数据库用户名
  password: password      # 数据库密码
  database: dbname        # 数据库名称
  ssl_mode: prefer        # SSL模式 (disable/prefer/require)
  max_pool_size: 10       # 连接池大小
```

### OpenAPI 配置

```yaml
openapi:
  spec:                   # OpenAPI 3.0 规范
    # ... OpenAPI 规范内容
  validation:
    strict_mode: true     # 严格验证模式
    validate_request_body: true    # 验证请求体
    validate_response_body: false # 验证响应体
```

## 架构设计

Apify 采用模块化设计：

- **配置层**: 解析 YAML 配置文件
- **API 生成层**: 根据 OpenAPI 规范生成路由
- **数据库层**: 处理 PostgreSQL 连接和查询
- **验证层**: OpenAPI 请求/响应验证
- **路由层**: HTTP 请求路由和参数提取

## 开发计划

- [ ] 支持更多数据库类型（MySQL、SQLite）
- [ ] 集成 Flow 模块支持复杂业务逻辑
- [ ] 添加数据缓存支持
- [ ] 支持批量操作
- [ ] 添加 API 文档自动生成
- [ ] 支持认证和授权

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

MIT License


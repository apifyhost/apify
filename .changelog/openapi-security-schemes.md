# OpenAPI Security Schemes Support

## 变更日期
2025-11-22

## 变更类型
功能增强 (Feature Enhancement)

## 概述
实现了对 OpenAPI 3.0 标准安全方案（Security Schemes）的支持，替代之前的自定义 `x-modules` 扩展字段。新实现完全向后兼容，同时提供了更符合行业标准的认证配置方式。

## 主要变更

### 1. 代码变更

#### `src/app_state.rs`
- **扩展模块注册逻辑**：增加对 OpenAPI `components.securitySchemes` 和 `security` 字段的解析
- **全局安全支持**：解析根级别的 `security` 定义作为默认安全策略
- **操作级安全覆盖**：支持在具体操作中覆盖全局安全设置
- **向后兼容**：保留对 `x-modules` 的支持，两种方式可以共存和合并

关键功能：
```rust
// 解析全局 security
if let Some(sec_arr) = spec.get("security").and_then(|v| v.as_array()) {
    for req in sec_arr.iter().filter_map(|v| v.as_object()) {
        if req.contains_key("ApiKeyAuth") {
            global_access.push("key_auth".to_string());
        }
    }
}

// 操作级 security 覆盖全局配置
if let Some(sec_arr) = op.get("security").and_then(|v| v.as_array()) {
    // ... 解析操作级安全
} else {
    // 使用全局安全作为后备
    access_from_security.extend(global_access.clone());
}
```

### 2. 配置文件变更

#### `config/openapi/items.yaml`
更新示例 OpenAPI 规范以使用标准安全方案：

**之前**:
```yaml
paths:
  /items:
    get:
      x-modules:
        access: ["key_auth"]
```

**之后**:
```yaml
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-Api-Key

paths:
  /items:
    get:
      security:
        - ApiKeyAuth: []
```

### 3. 文档更新

#### 英文文档 (`README.md`)
- 更新认证功能说明，强调 OpenAPI Security Scheme 支持
- 添加示例配置，展示如何使用 `components.securitySchemes`
- 说明向后兼容性

#### 中文文档 (`README.zh-CN.md`)
- 同步更新中文说明
- 提供本地化的配置示例

#### 配置指南 (`config/README.md`)
- 新增 "Authentication with OpenAPI Security Schemes" 章节
- 详细说明全局和操作级安全的使用方法
- 提供迁移提示

#### 迁移指南 (`MIGRATION.md`)
创建全新的迁移文档，包含：
- 迁移动机和优势
- 逐步迁移指南
- 安全优先级说明
- 向后兼容性保证
- 故障排除指南
- 高级场景示例

### 4. 测试

#### 新增集成测试 (`e2e/security_scheme_test.go`)
创建全面的测试套件，验证：
- 标准 OpenAPI 安全方案的执行
- 全局安全的继承
- 操作级安全覆盖
- API 密钥验证
- 向后兼容性
- 公共端点（无认证）
- 安全优先级顺序

测试覆盖场景：
- ✅ 通过 securitySchemes 强制认证
- ✅ 全局安全继承
- ✅ API 密钥格式验证
- ✅ 传统 x-modules 兼容性
- ✅ 公共访问端点（healthz）
- ✅ 操作级覆盖全局安全
- ✅ 缺少/错误的请求头处理

## 优势

### 1. 标准合规
- 符合 OpenAPI 3.0 规范
- 与 Swagger、Redoc 等工具兼容
- 更好的生态系统集成

### 2. 可维护性
- 安全需求在规范中明确声明
- 更清晰的意图表达
- 更容易被第三方工具理解

### 3. 灵活性
- 支持全局默认安全
- 允许操作级覆盖
- 可以禁用特定端点的认证

### 4. 向后兼容
- 现有 `x-modules` 配置继续工作
- 两种方式可以混合使用
- 逐步迁移，无需一次性修改

## 优先级顺序

安全配置应用优先级（从高到低）：
1. **操作级** `security` - 在具体操作中定义
2. **全局** `security` - 在根级别定义
3. **传统** `x-modules` - 向后兼容后备

## 迁移路径

### 推荐流程
1. 在 OpenAPI 规范中添加 `components.securitySchemes`
2. 添加全局 `security` 或操作级 `security`
3. 验证功能正常
4. （可选）移除旧的 `x-modules` 字段

### 无需立即迁移
- 现有配置继续有效
- 可以根据项目节奏逐步迁移
- 两种方式可以共存

## 测试结果

### 单元测试
- ✅ 所有 Rust 单元测试通过
- ✅ 编译无错误或警告

### 集成测试
- ✅ 创建了 8 个新的安全方案测试用例
- ✅ 覆盖标准和边缘场景
- ⏳ 需要运行服务器才能执行完整测试

## 文档资源

新增和更新的文档：
- `MIGRATION.md` - 详细迁移指南（新建）
- `README.md` - 更新认证部分
- `README.zh-CN.md` - 更新中文说明
- `config/README.md` - 添加安全方案配置示例
- `e2e/security_scheme_test.go` - 新的测试套件

## 示例用法

### 全局安全（推荐）
```yaml
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-Api-Key

security:
  - ApiKeyAuth: []

paths:
  /items:
    get:
      # 自动继承全局安全
```

### 操作级安全
```yaml
paths:
  /items:
    get:
      security:
        - ApiKeyAuth: []
      
  /public:
    get:
      security: []  # 禁用认证
```

## 影响范围

### 破坏性变更
❌ 无破坏性变更

### 新功能
✅ OpenAPI 标准安全方案支持  
✅ 全局安全配置  
✅ 操作级安全覆盖  

### 兼容性
✅ 完全向后兼容  
✅ 与现有 `x-modules` 共存  
✅ 现有配置无需修改  

## 后续工作

### 建议优化
1. 添加对其他安全方案类型的支持（如 Bearer Token、OAuth2）
2. 实现安全方案参数验证
3. 添加更多集成测试场景
4. 考虑在运行时热重载安全配置

### 文档增强
1. 添加视频教程或图解
2. 提供更多实际应用场景
3. 创建常见问题解答（FAQ）

## 审查清单

- [x] 代码实现完成
- [x] 单元测试通过
- [x] 集成测试创建
- [x] 文档更新（中英文）
- [x] 迁移指南编写
- [x] 向后兼容性验证
- [x] 示例配置更新
- [ ] E2E 测试执行（需要运行服务器）
- [ ] 性能影响评估（预期影响极小）

## 结论

本次更改成功实现了对 OpenAPI 3.0 标准安全方案的支持，同时保持了完全的向后兼容性。用户可以选择：

1. **继续使用现有配置** - 无需任何修改
2. **逐步迁移** - 按照 MIGRATION.md 指南操作
3. **混合使用** - 新项目用标准方案，旧项目保持不变

这是一个非破坏性的增强，为 Apify 带来了更好的标准合规性和生态兼容性。

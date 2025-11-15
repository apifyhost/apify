# Observability E2E Tests

可观测性功能的端到端测试。

## 测试覆盖

### Prometheus 指标测试
- ✅ 指标端点可用性
- ✅ HTTP 请求指标 (apify_http_requests_total, apify_http_request_duration_seconds)
- ✅ 活跃连接数 (apify_active_connections)
- ✅ 工作线程数 (apify_worker_threads)
- ✅ 数据库查询指标 (apify_db_queries_total, apify_db_query_duration_seconds)
- ✅ 状态码标签
- ✅ 直方图桶
- ✅ 方法和路径标签
- ✅ 高负载性能

### 结构化日志测试
- 📝 JSON 格式验证 (需要日志聚合系统)

### 健康检查
- ✅ 健康端点返回正确状态
- ✅ 健康检查被记录在指标中

## 快速开始

### 1. 启动服务

```bash
# 启动 SQLite 版本
docker compose up -d apify-sqlite

# 或启动 PostgreSQL 版本
docker compose up -d postgres apify-postgres
```

### 2. 运行测试

```bash
# 使用便捷脚本
cd e2e-tests
./test-observability.sh

# 或使用 make
make test-observability

# 或直接使用 ginkgo
BASE_URL=http://localhost:3000 METRICS_PORT=9090 ginkgo -v --focus="Observability"
```

### 3. PostgreSQL 测试

```bash
# PostgreSQL 使用不同端口
BASE_URL=http://localhost:3001 METRICS_PORT=9091 ./test-observability.sh
```

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `BASE_URL` | `http://localhost:3000` | Apify 服务地址 |
| `METRICS_PORT` | `9090` | Prometheus 指标端口 |
| `API_KEY` | `e2e-test-key-001` | API 认证密钥 |

## 测试场景

### 1. 指标端点可用性
验证 `/metrics` 端点返回 200 状态码和正确的 Content-Type。

### 2. HTTP 请求指标
生成多个 HTTP 请求后，验证以下指标存在：
- `apify_http_requests_total` - 请求计数器
- `apify_http_request_duration_seconds` - 延迟直方图
- `apify_active_connections` - 活跃连接数

### 3. 工作线程指标
验证 `apify_worker_threads` 显示正确的线程数（测试环境为 1）。

### 4. 状态码跟踪
发送不同状态码的请求（200, 404），验证指标中包含对应的 `status` 标签。

### 5. 数据库操作指标
执行 CRUD 操作后，验证数据库指标：
- `apify_db_queries_total` - 查询计数
- `apify_db_query_duration_seconds` - 查询延迟
- 包含 `operation` 和 `table` 标签

### 6. 直方图桶
验证延迟直方图包含预定义的桶：
- `le="0.001"` (1ms)
- `le="0.01"` (10ms)
- `le="0.1"` (100ms)
- `le="1"` (1s)
- `le="+Inf"`

### 7. 高负载性能
发送 50 个并发请求，验证：
- 指标端点仍然可用
- 所有预期指标都存在
- 响应时间在可接受范围内

### 8. 指标标签
验证指标包含正确的标签：
- `method` - HTTP 方法 (GET, POST, PUT, DELETE)
- `path` - 请求路径
- `status` - HTTP 状态码
- `operation` - 数据库操作类型
- `table` - 数据库表名

## CI/CD 集成

测试在 GitHub Actions 中自动运行：

```yaml
- name: Run observability tests
  working-directory: e2e-tests
  env:
    BASE_URL: http://localhost:3000
    API_KEY: e2e-test-key-001
    METRICS_PORT: 9090
  run: ginkgo -v --focus="Observability"
```

## 预期输出

成功运行的测试应该显示：

```
Running Suite: Apify E2E Test Suite
====================================

Observability Features
  Prometheus Metrics Endpoint
    ✓ should expose metrics endpoint
    ✓ should include HTTP request metrics
    ✓ should include worker threads gauge
    ✓ should track request counts by status code
    ✓ should include database query metrics after CRUD operations
    ✓ should include histogram buckets for request duration
  Health Check Endpoint
    ✓ should return healthy status
    ✓ should be included in metrics
  Metrics Performance
    ✓ should handle high request volume
    ✓ should report metrics quickly
  Active Connections Gauge
    ✓ should track active connections
  Metric Labels
    ✓ should include method labels
    ✓ should include path labels

Ran 13 of 13 Specs in 2.345 seconds
SUCCESS! -- 13 Passed | 0 Failed | 0 Pending | 0 Skipped
```

## 故障排除

### 指标端点不可用

```bash
# 检查服务是否启用了可观测性
docker compose logs apify-sqlite | grep -i observability

# 验证端口映射
docker compose ps

# 手动测试端点
curl http://localhost:9090/metrics
```

### 指标不包含预期数据

```bash
# 生成一些流量
for i in {1..10}; do curl http://localhost:3000/healthz; done

# 等待一会儿让指标更新
sleep 1

# 再次检查
curl http://localhost:9090/metrics | grep apify_
```

### 测试超时

增加超时时间或检查服务响应性：

```bash
# 检查服务健康
docker compose logs apify-sqlite --tail 50

# 重启服务
docker compose restart apify-sqlite
```

## 扩展测试

### 添加新的指标测试

```go
It("should track custom metric", func() {
    // 触发产生指标的操作
    resp, err := client.Get(baseURL + "/custom-endpoint")
    Expect(err).NotTo(HaveOccurred())
    resp.Body.Close()
    
    time.Sleep(100 * time.Millisecond)
    
    // 验证指标
    metricsResp, err := client.Get(metricsURL)
    Expect(err).NotTo(HaveOccurred())
    defer metricsResp.Body.Close()
    
    body, _ := io.ReadAll(metricsResp.Body)
    metricsText := string(body)
    
    Expect(metricsText).To(ContainSubstring("my_custom_metric"))
})
```

## 相关文档

- [可观测性完整文档](../observability/README.md)
- [快速开始指南](../observability/QUICKSTART.zh-CN.md)
- [Prometheus 查询示例](../observability/README.md#metrics)
- [Grafana 仪表板](../observability/grafana/dashboards/)

## 性能基准

预期性能指标（参考值）：

| 指标 | 预期值 |
|------|--------|
| 指标端点响应时间 | < 100ms |
| 50 并发请求处理时间 | < 5s |
| 指标开销 | < 1% CPU |
| 内存开销 | < 10MB |

## 许可证

与 Apify 项目相同。

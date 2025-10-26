#!/bin/bash

# Apify CRUD API 测试脚本

BASE_URL="http://localhost:3000"
echo "🚀 开始测试 Apify CRUD API..."

# 等待服务启动
echo "⏳ 等待服务启动..."
sleep 2

# 测试 1: 获取所有用户
echo "📋 测试 1: 获取所有用户"
curl -s "$BASE_URL/users" | jq '.' 2>/dev/null || echo "响应: $(curl -s "$BASE_URL/users")"
echo ""

# 测试 2: 创建新用户
echo "➕ 测试 2: 创建新用户"
CREATE_RESPONSE=$(curl -s -X POST "$BASE_URL/users" \
  -H "Content-Type: application/json" \
  -d '{"name": "测试用户", "email": "test@example.com"}')
echo "创建响应: $CREATE_RESPONSE"

# 提取用户ID（假设返回的是JSON格式）
USER_ID=$(echo "$CREATE_RESPONSE" | jq -r '.id' 2>/dev/null || echo "1")
echo "用户ID: $USER_ID"
echo ""

# 测试 3: 根据ID获取用户
echo "🔍 测试 3: 根据ID获取用户 ($USER_ID)"
curl -s "$BASE_URL/users/$USER_ID" | jq '.' 2>/dev/null || echo "响应: $(curl -s "$BASE_URL/users/$USER_ID")"
echo ""

# 测试 4: 更新用户
echo "✏️ 测试 4: 更新用户 ($USER_ID)"
curl -s -X PUT "$BASE_URL/users/$USER_ID" \
  -H "Content-Type: application/json" \
  -d '{"name": "测试用户（已更新）", "email": "test_updated@example.com"}' | jq '.' 2>/dev/null || echo "响应: $(curl -s -X PUT "$BASE_URL/users/$USER_ID" -H "Content-Type: application/json" -d '{"name": "测试用户（已更新）", "email": "test_updated@example.com"}')"
echo ""

# 测试 5: 验证更新
echo "✅ 测试 5: 验证更新"
curl -s "$BASE_URL/users/$USER_ID" | jq '.' 2>/dev/null || echo "响应: $(curl -s "$BASE_URL/users/$USER_ID")"
echo ""

# 测试 6: 分页查询
echo "📄 测试 6: 分页查询"
curl -s "$BASE_URL/users?limit=5&offset=0" | jq '.' 2>/dev/null || echo "响应: $(curl -s "$BASE_URL/users?limit=5&offset=0")"
echo ""

# 测试 7: 删除用户
echo "🗑️ 测试 7: 删除用户 ($USER_ID)"
curl -s -X DELETE "$BASE_URL/users/$USER_ID" | jq '.' 2>/dev/null || echo "响应: $(curl -s -X DELETE "$BASE_URL/users/$USER_ID")"
echo ""

# 测试 8: 验证删除
echo "🔍 测试 8: 验证删除（应该返回404）"
curl -s -w "HTTP状态码: %{http_code}\n" "$BASE_URL/users/$USER_ID" | jq '.' 2>/dev/null || echo "响应: $(curl -s -w "HTTP状态码: %{http_code}\n" "$BASE_URL/users/$USER_ID")"
echo ""

echo "🎉 测试完成！"
echo ""
echo "💡 提示："
echo "   - 确保 PostgreSQL 数据库正在运行"
echo "   - 确保数据库中有 users 表"
echo "   - 确保 Apify 服务正在运行在端口 3000"
echo "   - 如果没有安装 jq，请运行: brew install jq (macOS) 或 apt-get install jq (Ubuntu)"

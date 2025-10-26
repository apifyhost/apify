-- Apify 数据库初始化脚本
-- 创建示例数据库和表结构

-- 创建数据库（如果不存在）
-- 注意：这需要在 postgres 数据库中执行
-- CREATE DATABASE apify_db;

-- 连接到 apify_db 数据库
-- \c apify_db;

-- 创建用户表
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 创建索引以提高查询性能
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at);

-- 插入示例数据（如果表为空）
INSERT INTO users (name, email) 
SELECT '张三', 'zhangsan@example.com'
WHERE NOT EXISTS (SELECT 1 FROM users WHERE email = 'zhangsan@example.com');

INSERT INTO users (name, email) 
SELECT '李四', 'lisi@example.com'
WHERE NOT EXISTS (SELECT 1 FROM users WHERE email = 'lisi@example.com');

INSERT INTO users (name, email) 
SELECT '王五', 'wangwu@example.com'
WHERE NOT EXISTS (SELECT 1 FROM users WHERE email = 'wangwu@example.com');

-- 显示表结构
\d users;

-- 显示示例数据
SELECT * FROM users ORDER BY id;

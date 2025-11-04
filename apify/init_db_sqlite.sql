-- Apify SQLite database initialization script
-- Create example database and table structure

-- Create users table
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes to improve query performance
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at);

-- Insert sample data (if table is empty)
INSERT OR IGNORE INTO users (name, email) VALUES ('张三', 'zhangsan@example.com');
INSERT OR IGNORE INTO users (name, email) VALUES ('李四', 'lisi@example.com');
INSERT OR IGNORE INTO users (name, email) VALUES ('王五', 'wangwu@example.com');

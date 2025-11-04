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

-- Create books table
CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    author TEXT NOT NULL,
    isbn TEXT UNIQUE,
    published_date DATE,
    price REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for books table
CREATE INDEX IF NOT EXISTS idx_books_author ON books(author);
CREATE INDEX IF NOT EXISTS idx_books_isbn ON books(isbn);
CREATE INDEX IF NOT EXISTS idx_books_published_date ON books(published_date);

-- Insert sample books data (if table is empty)
INSERT OR IGNORE INTO books (title, author, isbn, published_date, price) VALUES 
('The Rust Programming Language', 'Steve Klabnik', '978-1593278284', '2018-08-01', 39.95),
('Programming Rust', 'Jim Blandy', '978-1449367425', '2015-08-27', 29.99),
('Rust in Action', 'Steve Donovan', '978-1617294551', '2017-09-19', 44.99);

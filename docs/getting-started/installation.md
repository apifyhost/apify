# Installation

## Prerequisites

- **Rust** 1.70 or higher (for building from source)
- **Docker** (recommended for quick start)
- **SQLite** (included) or **PostgreSQL** server

## From Source

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

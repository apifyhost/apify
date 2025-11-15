#!/bin/bash
# Prepare configuration for E2E testing
# Usage: ./prepare-config.sh [sqlite|postgres]

set -e

DB_TYPE=${1:-sqlite}
CONFIG_FILE="/app/config/config.yaml"

echo "Preparing E2E configuration for: $DB_TYPE"

case "$DB_TYPE" in
  sqlite)
    # Update datasource in config
    sed -i 's/datasource: postgres/datasource: sqlite/g' "$CONFIG_FILE"
    sed -i 's/datasource: default/datasource: sqlite/g' "$CONFIG_FILE"
    
    # Update SQLite database path for Docker environment
    sed -i 's|database: ./apify.sqlite|database: /app/data/e2e_test.sqlite|g' "$CONFIG_FILE"
    
    echo "✓ Configuration updated for SQLite"
    ;;
    
  postgres)
    # Update datasource in config
    sed -i 's/datasource: sqlite/datasource: postgres/g' "$CONFIG_FILE"
    sed -i 's/datasource: default/datasource: postgres/g' "$CONFIG_FILE"
    
    # Update PostgreSQL connection for Docker environment
    sed -i 's/host: localhost/host: postgres/g' "$CONFIG_FILE"
    sed -i 's/user: postgres/user: apify/g' "$CONFIG_FILE"
    sed -i 's/password: postgres/password: apify_test_password/g' "$CONFIG_FILE"
    sed -i 's/database: apify_db/database: apify_e2e/g' "$CONFIG_FILE"
    sed -i 's/ssl_mode: prefer/ssl_mode: disable/g' "$CONFIG_FILE"
    
    echo "✓ Configuration updated for PostgreSQL"
    ;;
    
  *)
    echo "Error: Unknown database type '$DB_TYPE'"
    echo "Usage: $0 [sqlite|postgres]"
    exit 1
    ;;
esac

echo "Configuration ready for E2E testing"

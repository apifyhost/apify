#!/bin/bash
# Apify Quickstart Script
# Quickly start Apify using Docker Compose with different examples

set -e

EXAMPLE="${1:-basic}"
EXAMPLE_DIR="examples/$EXAMPLE"

usage() {
  cat << EOF
Apify Quickstart - Run Apify examples via Docker Compose

Usage: $0 [EXAMPLE] [COMMAND]

Examples:
  basic           Basic CRUD API (default)
  oauth           OAuth/OIDC authentication with Keycloak
  observability   Observability with Prometheus, Grafana, and Jaeger
  full            Full-featured setup

Commands:
  start     Start the example (default)
  stop      Stop the example
  restart   Restart the example
  logs      View logs
  clean     Stop and remove all data

Usage Examples:
  $0                          # Start basic example
  $0 oauth                    # Start OAuth example
  $0 observability logs       # View logs of observability example
  $0 full stop                # Stop full example

For more information, see: https://github.com/apifyhost/apify
EOF
  exit 0
}

echo_fail() {
  printf "\e[31m✘ \033\e[0m$@\n"
}

echo_pass() {
  printf "\e[32m✔ \033\e[0m$@\n"
}

echo_warning() {
  printf "\e[33m⚠ \033\e[0m$@\n"
}

ensure_docker() {
  {
    docker compose version > /dev/null 2>&1
  } || {
    return 1
  }
}

download_example() {
  echo "Downloading example '$EXAMPLE' from GitHub..."
  
  if ! command -v curl >/dev/null 2>&1; then
    echo_fail "curl is required to download examples"
    exit 1
  fi
  if ! command -v tar >/dev/null 2>&1; then
    echo_fail "tar is required to download examples"
    exit 1
  fi

  mkdir -p examples
  
  # Download and extract specific folder
  # Note: apify-main is the default folder name in the archive for main branch
  if ! curl -fsSL https://github.com/apifyhost/apify/archive/main.tar.gz | \
       tar -xz -C examples --strip-components=2 "apify-main/examples/$EXAMPLE" 2>/dev/null; then
    echo_fail "Failed to download example '$EXAMPLE'. Please check your internet connection or example name."
    exit 1
  fi
  
  if [ ! -d "$EXAMPLE_DIR" ]; then
    echo_fail "Failed to download example '$EXAMPLE'. Directory not created."
    exit 1
  fi
}

check_example_dir() {
  if [ ! -d "$EXAMPLE_DIR" ]; then
    echo_warning "Example '$EXAMPLE' not found locally."
    download_example
  fi
}

start_example() {
  check_example_dir
  
  echo "Starting Apify ($EXAMPLE example)..."
  cd "$EXAMPLE_DIR"
  
  # For examples requiring external network (full)
  if [ "$EXAMPLE" = "full" ]; then
    docker network create apify_default 2>/dev/null || true
  fi
  
  docker compose up -d

  echo "Waiting for service to be ready..."
  if wait_for_service "Apify" "http://127.0.0.1:3000/healthz"; then
    echo_pass "Apify is ready!"
    output_info
    return 0
  else
    echo_fail "Failed to start Apify"
    docker compose logs
    return 1
  fi
}

stop_example() {
  check_example_dir
  
  echo "Stopping Apify ($EXAMPLE example)..."
  cd "$EXAMPLE_DIR"
  docker compose down
  echo_pass "Stopped"
}

restart_example() {
  check_example_dir
  
  echo "Restarting Apify ($EXAMPLE example)..."
  cd "$EXAMPLE_DIR"
  docker compose restart
  echo_pass "Restarted"
}

logs_example() {
  check_example_dir
  
  cd "$EXAMPLE_DIR"
  docker compose logs -f
}

clean_example() {
  check_example_dir
  
  echo "Cleaning Apify ($EXAMPLE example)..."
  cd "$EXAMPLE_DIR"
  docker compose down -v
  
  # Remove external network if exists
  if [ "$EXAMPLE" = "full" ]; then
    docker network rm apify_default 2>/dev/null || true
  fi
  
  echo_pass "Cleaned"
}

wait_for_service() {
  local service=$1
  local url=$2
  local retry_interval=2
  local retries=0
  local max_retry=30

  while [ $retries -lt $max_retry ]; do
    if curl -sf "$url" > /dev/null 2>&1; then
      return 0
    fi

    sleep $retry_interval
    ((retries+=1))
  done

  echo_fail "Timeout: Service $service is not available after ${max_retry} retries"
  return 1
}

output_info() {
  echo ""
  echo "🚀 Apify is running ($EXAMPLE example)!"
  echo ""
  echo "📍 Access points:"
  echo "   API:     http://localhost:3000"
  echo "   Health:  http://localhost:3000/healthz"
  echo "   Metrics: http://localhost:9090/metrics"
  
  case "$EXAMPLE" in
    oauth)
      echo ""
      echo "🔐 OAuth/Keycloak:"
      echo "   Keycloak Admin: http://localhost:8080 (admin/admin)"
      echo "   Realm: apify"
      ;;
    observability)
      echo ""
      echo "📊 Observability:"
      echo "   Prometheus:     http://localhost:9091"
      echo "   Grafana:        http://localhost:3001 (admin/admin)"
      echo "   Jaeger:         http://localhost:16686"
      ;;
    full)
      echo ""
      echo "🔐 OAuth/Keycloak:"
      echo "   Keycloak Admin: http://localhost:8080 (admin/admin)"
      echo ""
      echo "📊 Observability:"
      echo "   Prometheus:     http://localhost:9090"
      echo "   Grafana:        http://localhost:3002 (admin/admin)"
      echo "   Jaeger:         http://localhost:16686"
      echo ""
      echo "💾 Services:"
      echo "   PostgreSQL API: http://localhost:3000"
      echo "   SQLite API:     http://localhost:3001"
      ;;
  esac
  
  echo ""
  echo "💡 Quick commands:"
  echo "   View logs:  ./quickstart.sh $EXAMPLE logs"
  echo "   Stop:       ./quickstart.sh $EXAMPLE stop"
  echo "   Restart:    ./quickstart.sh $EXAMPLE restart"
  echo "   Clean:      ./quickstart.sh $EXAMPLE clean"
}

main() {
  # Parse arguments
  if [ "$1" = "help" ] || [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
    usage
  fi
  
  # If first arg looks like a command, use basic example
  local command="start"
  case "$1" in
    start|stop|restart|logs|clean)
      command="$1"
      EXAMPLE="basic"
      EXAMPLE_DIR="examples/$EXAMPLE"
      ;;
    basic|oauth|observability|full)
      EXAMPLE="$1"
      EXAMPLE_DIR="examples/$EXAMPLE"
      command="${2:-start}"
      ;;
    "")
      EXAMPLE="basic"
      EXAMPLE_DIR="examples/$EXAMPLE"
      command="start"
      ;;
    *)
      echo_fail "Unknown example or command: $1"
      echo ""
      usage
      ;;
  esac

  ensure_docker || {
    echo_fail "Docker is not available. Please install Docker and Docker Compose first"
    exit 1
  }

  case "$command" in
    start)
      start_example
      ;;
    stop)
      stop_example
      ;;
    restart)
      restart_example
      ;;
    logs)
      logs_example
      ;;
    clean)
      clean_example
      ;;
    *)
      echo_fail "Unknown command: $command"
      echo ""
      usage
      ;;
  esac
}

main "$@"

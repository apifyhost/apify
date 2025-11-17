#!/bin/bash
# Apify Quickstart Script
# Quickly start Apify using Docker Compose

set -e

RELEASE_TAG="${RELEASE_TAG:-0.1.0}"
DOWNLOAD_URL="https://github.com/apifyhost/apify/releases/download/${RELEASE_TAG}/apify-quickstart-${RELEASE_TAG}.tar.gz"
INSTALL_DIR="apify-quickstart-${RELEASE_TAG}"

usage() {
  cat << EOF
Apify Quickstart - Run Apify via Docker Compose

Usage: $0 [COMMAND]

Commands:
  install   Download and install Apify (default)
  start     Start Apify services
  stop      Stop Apify services
  destroy   Stop and remove Apify installation
  status    Check Apify service status
  help      Show this help message

Environment Variables:
  RELEASE_TAG    Release tag to download (default: quickstart-0.1.0)

Examples:
  $0                          # Install and start Apify
  $0 start                    # Start existing installation
  RELEASE_TAG=v0.2.0 $0       # Install specific version

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

ensure_curl() {
  {
    curl -h > /dev/null 2>&1
  } || {
    return 1
  }
}


install_apify() {
  # Check if running in a directory with existing files
  if [ -f "docker-compose.yml" ] && [ -f "quickstart.sh" ]; then
    echo_warning "Found existing docker-compose.yml in current directory"
    echo "Using local files instead of downloading..."
    
    echo "Starting Apify..."
    docker compose up -d

    echo "Waiting for service to be ready..."
    if wait_for_service "Apify" "http://127.0.0.1:3000/healthz"; then
      echo_pass "Apify is ready!"
      return 0
    else
      echo_fail "Failed to start Apify"
      echo "Checking logs..."
      docker compose logs
      return 1
    fi
  fi

  # Check if INSTALL_DIR already exists
  if [ -d "$INSTALL_DIR" ]; then
    echo_warning "Found existing installation directory: $INSTALL_DIR"
    echo -n "Do you want to use it? [Y/n] "
    read -r response
    case "$response" in
      [nN][oO]|[nN])
        echo "Continuing with fresh download..."
        rm -rf "$INSTALL_DIR"
        ;;
      *)
        echo "Using existing installation..."
        cd "$INSTALL_DIR"

        echo "Starting Apify..."
        docker compose up -d

        echo "Waiting for service to be ready..."
        if wait_for_service "Apify" "http://127.0.0.1:3000/healthz"; then
          echo_pass "Apify is ready!"
          return 0
        else
          echo_fail "Failed to start Apify"
          echo "Checking logs..."
          docker compose logs
          return 1
        fi
        ;;
    esac
  fi

  echo "Downloading Apify ${RELEASE_TAG}..."

  if ! curl -fSL "$DOWNLOAD_URL" -o "${RELEASE_TAG}.tar.gz"; then
    echo_fail "Failed to download Apify from ${DOWNLOAD_URL}"
    echo_warning "You can also run this script in a directory with existing docker-compose.yml"
    return 1
  fi

  echo_pass "Downloaded successfully"

  echo "Extracting package..."
  mkdir -p "$INSTALL_DIR"
  tar -xzf "${RELEASE_TAG}.tar.gz" -C "$INSTALL_DIR"
  rm "${RELEASE_TAG}.tar.gz"

  echo_pass "Extracted to $INSTALL_DIR"

  cd "$INSTALL_DIR"

  echo "Starting Apify..."
  docker compose up -d

  echo "Waiting for service to be ready..."
  if wait_for_service "Apify" "http://127.0.0.1:3000/healthz"; then
    echo_pass "Apify is ready!"
    return 0
  else
    echo_fail "Failed to start Apify"
    echo "Checking logs..."
    docker compose logs
    return 1
  fi
}

start_apify() {
  if [ -d "$INSTALL_DIR" ]; then
    cd "$INSTALL_DIR"
  elif [ ! -f "docker-compose.yml" ]; then
    echo_fail "No Apify installation found. Run '$0 install' first."
    return 1
  fi

  echo "Starting Apify..."
  docker compose up -d

  if wait_for_service "Apify" "http://127.0.0.1:3000/healthz"; then
    echo_pass "Apify started successfully!"
    output_listen_address
    return 0
  else
    echo_fail "Failed to start Apify"
    docker compose logs
    return 1
  fi
}

stop_apify() {
  if [ -d "$INSTALL_DIR" ]; then
    cd "$INSTALL_DIR"
  elif [ ! -f "docker-compose.yml" ]; then
    echo_warning "No Apify installation found"
    return 0
  fi
  
  echo "Stopping Apify..."
  docker compose down
  echo_pass "Apify stopped"
}

destroy_apify() {
  if [ -d "$INSTALL_DIR" ]; then
    echo "Stopping and removing Apify installation..."
    cd "$INSTALL_DIR"
    docker compose down -v
    cd ..
    rm -rf "$INSTALL_DIR"
    echo_pass "Apify installation removed"
  else
    echo_warning "No installation directory found: $INSTALL_DIR"
  fi
}

status_apify() {
  if [ -d "$INSTALL_DIR" ]; then
    cd "$INSTALL_DIR"
  elif [ ! -f "docker-compose.yml" ]; then
    echo_fail "No Apify installation found"
    return 1
  fi

  echo "Apify service status:"
  docker compose ps

  echo ""
  echo "Testing health endpoint..."
  if curl -sf http://127.0.0.1:3000/healthz > /dev/null 2>&1; then
    echo_pass "Apify is healthy and responding"
    output_listen_address
  else
    echo_fail "Apify is not responding"
  fi
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

output_listen_address() {
  echo ""
  echo "🚀 Apify is running!"
  echo ""
  echo "📍 Access points:"
  echo "   API:     http://localhost:3000"
  echo "   Health:  http://localhost:3000/healthz"
  echo "   Metrics: http://localhost:9090/metrics"
  echo ""

  if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    ips=$(ip -4 addr | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | grep -v 127.0.0.1 | head -3)
  elif [[ "$OSTYPE" == "darwin"* ]]; then
    ips=$(ifconfig | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1' | head -3)
  fi

  if [ -n "$ips" ]; then
    echo "🌐 Network addresses:"
    for ip in $ips; do
      echo "   http://$ip:3000"
    done
    echo ""
  fi

  echo "💡 Quick commands:"
  echo "   View logs:  docker compose logs -f"
  echo "   Stop:       docker compose down"
  echo "   Restart:    docker compose restart"
}

main() {
  local command="${1:-install}"

  case "$command" in
    help|-h|--help)
      usage
      ;;
    install)
      ensure_docker || {
        echo_fail "Docker is not available. Please install Docker and Docker Compose first"
        exit 1
      }
      ensure_curl || {
        echo_fail "curl is not available. Please install curl first"
        exit 1
      }
      install_apify
      ;;
    start)
      ensure_docker || {
        echo_fail "Docker is not available"
        exit 1
      }
      start_apify
      ;;
    stop)
      ensure_docker || {
        echo_fail "Docker is not available"
        exit 1
      }
      stop_apify
      ;;
    destroy)
      ensure_docker || {
        echo_fail "Docker is not available"
        exit 1
      }
      destroy_apify
      ;;
    status)
      ensure_docker || {
        echo_fail "Docker is not available"
        exit 1
      }
      ensure_curl || {
        echo_fail "curl is not available"
        exit 1
      }
      status_apify
      ;;
    *)
      echo_fail "Unknown command: $command"
      echo ""
      usage
      ;;
  esac
}

main "$@"

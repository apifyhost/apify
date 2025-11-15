.PHONY: help deps test test-quick test-verbose test-observability test-crud test-all clean

help: ## Show this help message
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

deps: ## Install dependencies
	@echo "Installing Go dependencies..."
	go mod download
	go install github.com/onsi/ginkgo/v2/ginkgo@latest

test: ## Run all Go tests
	@echo "Running E2E tests..."
	./test.sh go

test-quick: ## Run quick smoke tests (curl-based)
	@echo "Running quick smoke tests..."
	./test.sh quick

test-verbose: ## Run tests with detailed output
	@echo "Running E2E tests with verbose output..."
	ginkgo -v

test-observability: ## Run observability tests only
	@echo "Running observability tests..."
	./test.sh observability

test-crud: ## Run CRUD tests only
	@echo "Running CRUD tests..."
	./test.sh crud

test-all: ## Run all tests with full configuration
	@echo "Running all tests..."
	BASE_URL=$${BASE_URL:-http://localhost:3000} \
	METRICS_PORT=$${METRICS_PORT:-9090} \
	ginkgo -v

clean: ## Clean test artifacts
	@echo "Cleaning test artifacts..."
	rm -rf reports/
	go clean -testcache

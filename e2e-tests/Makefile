.PHONY: help deps test test-verbose clean

help: ## Show this help message
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

deps: ## Install dependencies
	@echo "Installing Go dependencies..."
	go mod download
	go install github.com/onsi/ginkgo/v2/ginkgo@latest

test: ## Run tests
	@echo "Running E2E tests..."
	go test -v

test-verbose: ## Run tests with detailed output
	@echo "Running E2E tests with verbose output..."
	ginkgo -v

clean: ## Clean test artifacts
	@echo "Cleaning test artifacts..."
	rm -rf reports/
	go clean -testcache

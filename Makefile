# Neural Trader Phase 3 - Production Validation Makefile
# ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS

.PHONY: help install-hooks validate validate-all validate-quick validate-production
.PHONY: test test-coverage build clean format lint security
.PHONY: docker-build docker-test reports setup

# Configuration
RUST_TOOLCHAIN ?= stable
PYTHON_VERSION ?= 3.11
MIN_COVERAGE ?= 95
VALIDATION_MODE ?= development

# Colors for output
BOLD := \033[1m
RED := \033[31m
GREEN := \033[32m
YELLOW := \033[33m
BLUE := \033[34m
NC := \033[0m

# Help target
help: ## Show this help message
	@echo -e "$(BOLD)$(BLUE)Neural Trader Phase 3 - Production Validation$(NC)"
	@echo -e "$(BOLD)ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS$(NC)"
	@echo ""
	@echo -e "$(BOLD)Available targets:$(NC)"
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ { printf "  $(BLUE)%-20s$(NC) %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@echo ""
	@echo -e "$(BOLD)Examples:$(NC)"
	@echo -e "  make validate              # Run development validation"
	@echo -e "  make validate-production   # Run production validation (ZERO TOLERANCE)"
	@echo -e "  make test-coverage         # Run tests with coverage reporting"
	@echo -e "  make install-hooks         # Install git hooks for continuous validation"

# Setup and installation
setup: ## Setup development environment
	@echo -e "$(BOLD)$(BLUE)Setting up Neural Trader development environment...$(NC)"
	@rustup toolchain install $(RUST_TOOLCHAIN)
	@rustup component add rustfmt clippy
	@cargo install cargo-tarpaulin cargo-audit cargo-deny
	@python3 -m pip install --upgrade pip
	@pip install -r requirements.txt
	@pip install pytest-cov bandit safety black isort mypy
	@echo -e "$(GREEN)✅ Development environment setup complete$(NC)"

install-hooks: ## Install git hooks for continuous validation
	@echo -e "$(BOLD)$(BLUE)Installing git hooks...$(NC)"
	@./scripts/validation/install-hooks.sh --install
	@echo -e "$(GREEN)✅ Git hooks installed$(NC)"

# Validation targets
validate: ## Run development validation (basic checks)
	@echo -e "$(BOLD)$(BLUE)Running development validation...$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=all --mode=development
	@echo -e "$(GREEN)✅ Development validation complete$(NC)"

validate-quick: ## Run quick validation (code completeness only)
	@echo -e "$(BOLD)$(BLUE)Running quick validation...$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=code-completeness --mode=development
	@echo -e "$(GREEN)✅ Quick validation complete$(NC)"

validate-production: ## Run full production validation (ZERO TOLERANCE)
	@echo -e "$(BOLD)$(YELLOW)⚠️  RUNNING PRODUCTION VALIDATION - ZERO TOLERANCE MODE$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=all --mode=production --fail-fast
	@echo -e "$(GREEN)🚀 PRODUCTION VALIDATION PASSED - DEPLOYMENT APPROVED$(NC)"

validate-staging: ## Run staging validation
	@echo -e "$(BOLD)$(BLUE)Running staging validation...$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=all --mode=staging
	@echo -e "$(GREEN)✅ Staging validation complete$(NC)"

validate-interface: ## Validate interface contracts only
	@echo -e "$(BOLD)$(BLUE)Validating interface contracts...$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=interface-contract --mode=$(VALIDATION_MODE)

validate-coverage: ## Validate test coverage only
	@echo -e "$(BOLD)$(BLUE)Validating test coverage...$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=test-coverage --mode=$(VALIDATION_MODE)

validate-performance: ## Validate performance benchmarks only
	@echo -e "$(BOLD)$(BLUE)Validating performance benchmarks...$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=performance-benchmark --mode=$(VALIDATION_MODE)

validate-security: ## Validate security standards only
	@echo -e "$(BOLD)$(BLUE)Validating security standards...$(NC)"
	@./scripts/validation/run-production-validation.sh --validator=security-standards --mode=$(VALIDATION_MODE)

# Build targets
build: ## Build all binaries
	@echo -e "$(BOLD)$(BLUE)Building all binaries...$(NC)"
	@cargo build --release --bin config-store
	@cargo build --release --bin ruv-fann
	@cargo build --release --bin daa-coordinator
	@cargo build --release --bin production-validator
	@echo -e "Building Python data-ingestion binary..."
	@cd data-ingestion && python -m compileall .
	@echo -e "$(GREEN)✅ All binaries built successfully$(NC)"

build-debug: ## Build all binaries in debug mode
	@echo -e "$(BOLD)$(BLUE)Building debug binaries...$(NC)"
	@cargo build --bin config-store
	@cargo build --bin ruv-fann
	@cargo build --bin daa-coordinator
	@cargo build --bin production-validator
	@echo -e "$(GREEN)✅ Debug binaries built$(NC)"

# Test targets
test: ## Run all tests
	@echo -e "$(BOLD)$(BLUE)Running all tests...$(NC)"
	@cargo test --all
	@cd data-ingestion && python -m pytest
	@echo -e "$(GREEN)✅ All tests passed$(NC)"

test-rust: ## Run Rust tests only
	@echo -e "$(BOLD)$(BLUE)Running Rust tests...$(NC)"
	@cargo test --all
	@echo -e "$(GREEN)✅ Rust tests passed$(NC)"

test-python: ## Run Python tests only
	@echo -e "$(BOLD)$(BLUE)Running Python tests...$(NC)"
	@cd data-ingestion && python -m pytest -v
	@echo -e "$(GREEN)✅ Python tests passed$(NC)"

test-coverage: ## Run tests with coverage reporting (minimum 95%)
	@echo -e "$(BOLD)$(BLUE)Running tests with coverage (minimum $(MIN_COVERAGE)%)...$(NC)"
	@echo -e "$(BLUE)Rust coverage:$(NC)"
	@cargo tarpaulin --out Html --output-dir target/coverage/rust/ --minimum $(MIN_COVERAGE)
	@echo -e "$(BLUE)Python coverage:$(NC)"
	@cd data-ingestion && python -m pytest --cov=. --cov-report=html:../target/coverage/python/ --cov-fail-under=$(MIN_COVERAGE)
	@echo -e "$(GREEN)✅ Coverage requirements met (≥$(MIN_COVERAGE)%)$(NC)"

test-integration: ## Run integration tests
	@echo -e "$(BOLD)$(BLUE)Running integration tests...$(NC)"
	@cargo test --test '*integration*'
	@echo -e "$(GREEN)✅ Integration tests passed$(NC)"

test-benchmark: ## Run performance benchmarks
	@echo -e "$(BOLD)$(BLUE)Running performance benchmarks...$(NC)"
	@cargo bench
	@echo -e "$(GREEN)✅ Performance benchmarks complete$(NC)"

# Code quality targets
format: ## Format all code
	@echo -e "$(BOLD)$(BLUE)Formatting code...$(NC)"
	@cargo fmt --all
	@black .
	@isort .
	@echo -e "$(GREEN)✅ Code formatted$(NC)"

format-check: ## Check code formatting
	@echo -e "$(BOLD)$(BLUE)Checking code formatting...$(NC)"
	@cargo fmt --all -- --check
	@black --check .
	@isort --check-only .
	@echo -e "$(GREEN)✅ Code formatting is correct$(NC)"

lint: ## Run linting
	@echo -e "$(BOLD)$(BLUE)Running linting...$(NC)"
	@cargo clippy --all-targets --all-features -- -D warnings
	@cd data-ingestion && python -m flake8 .
	@cd data-ingestion && python -m mypy .
	@echo -e "$(GREEN)✅ Linting passed$(NC)"

lint-fix: ## Run linting with auto-fixes
	@echo -e "$(BOLD)$(BLUE)Running linting with fixes...$(NC)"
	@cargo clippy --all-targets --all-features --fix -- -D warnings
	@echo -e "$(GREEN)✅ Linting fixes applied$(NC)"

# Security targets
security: ## Run security audits
	@echo -e "$(BOLD)$(BLUE)Running security audits...$(NC)"
	@cargo audit
	@cargo deny check
	@cd data-ingestion && python -m bandit -r . -ll
	@cd data-ingestion && python -m safety check
	@echo -e "$(GREEN)✅ Security audits passed$(NC)"

security-rust: ## Run Rust security audit only
	@echo -e "$(BOLD)$(BLUE)Running Rust security audit...$(NC)"
	@cargo audit
	@cargo deny check
	@echo -e "$(GREEN)✅ Rust security audit passed$(NC)"

security-python: ## Run Python security audit only
	@echo -e "$(BOLD)$(BLUE)Running Python security audit...$(NC)"
	@cd data-ingestion && python -m bandit -r . -ll
	@cd data-ingestion && python -m safety check
	@echo -e "$(GREEN)✅ Python security audit passed$(NC)"

# Docker targets
docker-build: ## Build Docker images for all binaries
	@echo -e "$(BOLD)$(BLUE)Building Docker images...$(NC)"
	@docker build -f docker/config-store.Dockerfile -t neural-trader/config-store .
	@docker build -f docker/data-ingestion.Dockerfile -t neural-trader/data-ingestion .
	@docker build -f docker/ruv-fann.Dockerfile -t neural-trader/ruv-fann .
	@docker build -f docker/daa-coordinator.Dockerfile -t neural-trader/daa-coordinator .
	@echo -e "$(GREEN)✅ Docker images built$(NC)"

docker-test: ## Run tests in Docker containers
	@echo -e "$(BOLD)$(BLUE)Running tests in Docker...$(NC)"
	@docker-compose -f docker/test-compose.yml up --build --abort-on-container-exit
	@echo -e "$(GREEN)✅ Docker tests completed$(NC)"

docker-validate: ## Run validation in Docker environment
	@echo -e "$(BOLD)$(BLUE)Running validation in Docker...$(NC)"
	@docker run --rm -v $(PWD):/workspace neural-trader/validator \
		./scripts/validation/run-production-validation.sh --validator=all --mode=production
	@echo -e "$(GREEN)✅ Docker validation completed$(NC)"

# Report generation targets
reports: ## Generate all validation reports
	@echo -e "$(BOLD)$(BLUE)Generating validation reports...$(NC)"
	@mkdir -p target/reports
	@./scripts/validation/run-production-validation.sh --validator=all --mode=development --report=html --output=target/reports
	@./scripts/validation/run-production-validation.sh --validator=all --mode=development --report=json --output=target/reports
	@echo -e "$(GREEN)✅ Reports generated in target/reports/$(NC)"

reports-production: ## Generate production validation reports
	@echo -e "$(BOLD)$(BLUE)Generating production validation reports...$(NC)"
	@mkdir -p target/reports/production
	@./scripts/validation/run-production-validation.sh --validator=all --mode=production --report=html --output=target/reports/production
	@echo -e "$(GREEN)✅ Production reports generated$(NC)"

# CI/CD simulation targets
ci-test: ## Simulate CI/CD pipeline locally
	@echo -e "$(BOLD)$(YELLOW)🔍 Simulating CI/CD pipeline locally...$(NC)"
	@echo -e "$(BLUE)Step 1: Pre-commit validation$(NC)"
	@make validate-quick
	@echo -e "$(BLUE)Step 2: Build validation$(NC)"
	@make build
	@echo -e "$(BLUE)Step 3: Test validation$(NC)"
	@make test
	@echo -e "$(BLUE)Step 4: Coverage validation$(NC)"
	@make test-coverage
	@echo -e "$(BLUE)Step 5: Security validation$(NC)"
	@make security
	@echo -e "$(BLUE)Step 6: Final production validation$(NC)"
	@make validate-production
	@echo -e "$(GREEN)🚀 CI/CD simulation PASSED - Ready for deployment!$(NC)"

pre-commit: ## Run pre-commit checks locally
	@echo -e "$(BOLD)$(BLUE)Running pre-commit checks...$(NC)"
	@./scripts/validation/pre-commit-hook.sh
	@echo -e "$(GREEN)✅ Pre-commit checks passed$(NC)"

pre-push: ## Run pre-push validation
	@echo -e "$(BOLD)$(BLUE)Running pre-push validation...$(NC)"
	@make validate
	@make test
	@echo -e "$(GREEN)✅ Pre-push validation passed$(NC)"

# Cleanup targets
clean: ## Clean build artifacts
	@echo -e "$(BOLD)$(BLUE)Cleaning build artifacts...$(NC)"
	@cargo clean
	@rm -rf target/coverage/
	@rm -rf target/reports/
	@find . -type d -name "__pycache__" -exec rm -rf {} + || true
	@find . -type f -name "*.pyc" -delete || true
	@echo -e "$(GREEN)✅ Clean complete$(NC)"

clean-all: ## Clean everything including dependencies
	@echo -e "$(BOLD)$(BLUE)Deep cleaning...$(NC)"
	@make clean
	@rm -rf ~/.cargo/registry/cache/
	@pip cache purge
	@echo -e "$(GREEN)✅ Deep clean complete$(NC)"

# Development workflow shortcuts
dev-setup: setup install-hooks ## Complete development setup
	@echo -e "$(BOLD)$(GREEN)🚀 Development environment ready!$(NC)"
	@echo -e "$(YELLOW)💡 Try: make validate-quick$(NC)"

dev-validate: format lint validate-quick ## Quick development validation
	@echo -e "$(BOLD)$(GREEN)✅ Development validation complete$(NC)"

dev-test: test-coverage ## Development testing with coverage
	@echo -e "$(BOLD)$(GREEN)✅ Development testing complete$(NC)"

# Production deployment workflow
production-check: format-check lint security validate-production ## Full production readiness check
	@echo -e "$(BOLD)$(GREEN)🚀 PRODUCTION DEPLOYMENT APPROVED$(NC)"

staging-deploy: validate-staging ## Staging deployment validation
	@echo -e "$(BOLD)$(GREEN)✅ Ready for staging deployment$(NC)"

# Status and information targets
status: ## Show project status
	@echo -e "$(BOLD)$(BLUE)Neural Trader Phase 3 Status$(NC)"
	@echo -e "$(BLUE)Project Root:$(NC) $(PWD)"
	@echo -e "$(BLUE)Git Branch:$(NC) $$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'unknown')"
	@echo -e "$(BLUE)Git Commit:$(NC) $$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
	@echo -e "$(BLUE)Rust Toolchain:$(NC) $$(rustc --version 2>/dev/null || echo 'not installed')"
	@echo -e "$(BLUE)Python Version:$(NC) $$(python3 --version 2>/dev/null || echo 'not installed')"
	@echo -e "$(BLUE)Git Hooks:$(NC) $$([ -x .git/hooks/pre-commit ] && echo 'installed' || echo 'not installed')"

version: ## Show version information
	@echo -e "$(BOLD)$(BLUE)Neural Trader Phase 3 - Version Information$(NC)"
	@echo "Project: Neural Trader Phase 3"
	@echo "Version: 3.0.0-alpha"
	@echo "Build: $$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
	@echo "Validation Framework: Production Ready (ZERO TOLERANCE)"

# Default target
.DEFAULT_GOAL := help
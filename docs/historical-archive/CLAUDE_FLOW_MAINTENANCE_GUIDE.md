# Claude-Flow Maintenance Guide for Neural-Trader Repository

## 🚨 CRITICAL: Repository Maintenance Instructions for AI Developer

This guide provides comprehensive instructions for Claude-Flow (the AI developer) to properly maintain and organize the neural-trader repository. Follow these rules exactly to ensure code quality and repository health.

## 📋 Table of Contents

1. [Repository Structure Standards](#repository-structure-standards)
2. [Code Organization Rules](#code-organization-rules)
3. [Documentation Requirements](#documentation-requirements)
4. [File Management Protocols](#file-management-protocols)
5. [Testing Standards](#testing-standards)
6. [Technical Debt Management](#technical-debt-management)
7. [Continuous Maintenance Workflow](#continuous-maintenance-workflow)

## 🏗️ Repository Structure Standards

### MANDATORY Directory Organization

```
neural-trader/
├── src/                    # Core Rust application code
│   ├── adapters/          # External service integrations
│   ├── neural/            # Neural network implementations
│   ├── integration/       # DAA and coordination systems
│   ├── strategies/        # Trading strategies
│   ├── monitoring/        # Health and performance monitoring
│   └── lib.rs            # Main library entry point
├── data_ingestion/        # Python data collection service
│   ├── providers/         # Market data providers
│   ├── utils/            # Utility functions
│   └── main.py           # Service entry point
├── tests/                 # Test suites
│   ├── unit/             # Unit tests
│   └── integration/      # Integration tests
├── docs/                  # User documentation
├── config/               # Configuration files
├── docker/               # Docker configurations
├── scripts/              # Utility scripts
└── models/               # Trained model storage
```

### ✅ RULES for Directory Structure:

1. **NEVER** create files in the root directory unless absolutely necessary
2. **ALWAYS** place new code in the appropriate subdirectory
3. **CONSOLIDATE** related files into logical modules
4. **DELETE** empty or obsolete directories
5. **MAINTAIN** a maximum directory depth of 4 levels

## 📁 Code Organization Rules

### 1. Module Organization

**MANDATORY Actions:**
- Group related functionality into modules (max 500 lines per file)
- Create a `mod.rs` file for each directory with proper exports
- Use descriptive module names that reflect functionality
- Implement proper visibility controls (pub/private)

**Example Structure:**
```rust
// src/neural/mod.rs
pub mod enhanced_predictor;
pub mod batch_optimizer;
pub mod ensemble_types;

pub use enhanced_predictor::EnhancedPredictor;
pub use batch_optimizer::BatchOptimizer;
```

### 2. File Naming Conventions

**STRICT Rules:**
- Use lowercase with underscores: `neural_predictor.rs`
- Test files: `<module>_test.rs` or in `tests/` directory
- Configuration: `<environment>.toml` (e.g., `production.toml`)
- Scripts: descriptive names with underscores

### 3. Code Duplication Prevention

**MANDATORY Actions:**
- Identify and consolidate duplicate code immediately
- Create shared utility modules for common functionality
- Use traits for shared behavior across types
- Implement generic functions where appropriate

**Current Issues to Fix:**
- Multiple feature engineering implementations
- Duplicate data conversion logic
- Redundant error handling patterns

## 📝 Documentation Requirements

### 1. Code Documentation Standards

**EVERY file MUST have:**
```rust
//! Module-level documentation explaining purpose
//! 
//! # Examples
//! ```rust
//! // Usage example
//! ```

/// Function documentation with:
/// - Purpose
/// - Parameters
/// - Return value
/// - Errors
pub fn example_function() -> Result<()> {
    // Implementation
}
```

### 2. README Files

**MANDATORY for each major directory:**
- Purpose and functionality overview
- Setup instructions
- API documentation
- Example usage
- Dependencies

### 3. Architecture Documentation

**MAINTAIN these documents:**
- `ARCHITECTURE.md` - System design overview
- `API_DOCUMENTATION.md` - API specifications
- `DEPLOYMENT_GUIDE.md` - Deployment instructions
- `TROUBLESHOOTING.md` - Common issues and solutions

## 🗂️ File Management Protocols

### 1. File Consolidation Rules

**IMMEDIATE Actions Required:**

1. **Reports and Analysis Files:**
   - Move ALL `*_REPORT.md`, `*_ANALYSIS.md` files to `docs/reports/`
   - Create subdirectories by date: `docs/reports/2025-01/`
   - Keep only the latest version in the main directory

2. **Configuration Files:**
   - Consolidate all Docker files into `docker/` subdirectories
   - Move environment-specific configs to `config/<environment>/`
   - Remove duplicate configuration files

3. **Build Artifacts:**
   - Add ALL build outputs to `.gitignore`
   - Clean up `*.txt`, `*.json` temporary files
   - Move logs to `logs/` directory (git-ignored)

### 2. File Deletion Criteria

**DELETE files that are:**
- Temporary analysis or report files older than 30 days
- Duplicate implementations with no unique functionality
- Empty placeholder files
- Obsolete documentation for removed features
- Build artifacts and compilation outputs

### 3. Archive Strategy

**ARCHIVE (don't delete) files that are:**
- Historical implementation references
- Migration guides for completed migrations
- Old architecture decisions (move to `docs/archive/`)

## 🧪 Testing Standards

### 1. Test Organization

**MANDATORY Structure:**
```
tests/
├── unit/              # Unit tests for individual components
├── integration/       # Integration tests
├── e2e/              # End-to-end tests
├── fixtures/         # Test data and mocks
└── common/           # Shared test utilities
```

### 2. Test Naming Conventions

**STRICT Rules:**
- Test function names: `test_<module>_<functionality>_<scenario>`
- Test files: `<module>_test.rs`
- Use descriptive names that explain what is being tested

### 3. Test Coverage Requirements

**MAINTAIN:**
- Minimum 80% code coverage for core modules
- 100% coverage for critical trading logic
- Integration tests for all external service interactions
- Performance benchmarks for neural network operations

## 🔧 Technical Debt Management

### 1. TODO/FIXME Protocol

**MANDATORY Format:**
```rust
// TODO(priority-date): Clear description of what needs to be done
// FIXME(critical-2025-01-30): Specific issue that needs immediate attention
```

**Current TODOs to Address:**
1. Fix circular dependency in DAA modules
2. Implement proper error propagation in neural adapters
3. Consolidate duplicate feature engineering code
4. Add missing integration tests for TimescaleDB adapter

### 2. Code Quality Metrics

**MONITOR and MAINTAIN:**
- Cyclomatic complexity < 10 per function
- File size < 500 lines
- Function size < 50 lines
- Module coupling < 5 dependencies

### 3. Refactoring Priority

**HIGH Priority Refactoring:**
1. Split large modules (neural/mod.rs, integration/mod.rs)
2. Extract common patterns into utility modules
3. Implement proper error types instead of string errors
4. Consolidate configuration loading logic

## 🔄 Continuous Maintenance Workflow

### Daily Maintenance Tasks

1. **Code Cleanup:**
   ```bash
   # Remove build artifacts
   cargo clean
   
   # Format code
   cargo fmt --all
   
   # Check for issues
   cargo clippy -- -D warnings
   ```

2. **Documentation Check:**
   - Verify all new code has documentation
   - Update README files for any API changes
   - Ensure examples are up-to-date

3. **Test Verification:**
   ```bash
   # Run all tests
   cargo test --all-features
   
   # Check coverage
   cargo tarpaulin --out Html
   ```

### Weekly Maintenance Tasks

1. **Dependency Management:**
   ```bash
   # Check for outdated dependencies
   cargo outdated
   
   # Update minor versions
   cargo update
   
   # Audit for security issues
   cargo audit
   ```

2. **Code Analysis:**
   - Review and address all TODO/FIXME comments
   - Identify and refactor code smells
   - Consolidate duplicate implementations

3. **Documentation Review:**
   - Update architecture diagrams
   - Review and improve API documentation
   - Archive obsolete documentation

### Monthly Maintenance Tasks

1. **Repository Cleanup:**
   - Archive old reports and analyses
   - Remove obsolete feature branches
   - Clean up unused Docker images
   - Optimize git history if needed

2. **Performance Review:**
   - Run benchmarks and compare with baseline
   - Profile memory usage
   - Optimize slow code paths
   - Update performance documentation

3. **Architecture Review:**
   - Assess module boundaries
   - Identify coupling issues
   - Plan refactoring for technical debt
   - Update architecture documentation

## 🚨 Critical Issues to Address Immediately

### 1. Repository Organization Issues

**IMMEDIATE Actions:**
- Move 50+ markdown files from root to appropriate subdirectories
- Consolidate 20+ Docker-related files into `docker/` structure
- Clean up build artifacts (*.txt, *.json files)
- Organize scripts into categorized subdirectories

### 2. Code Quality Issues

**HIGH Priority Fixes:**
- Resolve circular dependencies in DAA modules
- Fix inconsistent error handling patterns
- Consolidate duplicate data conversion implementations
- Implement proper logging standards

### 3. Documentation Gaps

**MUST Complete:**
- Document all public APIs with examples
- Create troubleshooting guide for common issues
- Write deployment guide for production setup
- Document model training and evaluation process

## 📊 Maintenance Metrics

### Track These Metrics Weekly:

1. **Code Health:**
   - Lines of code per module
   - Cyclomatic complexity
   - Test coverage percentage
   - Number of TODO/FIXME items

2. **Repository Health:**
   - Number of files in root directory (target: < 10)
   - Average file size (target: < 300 lines)
   - Documentation coverage (target: 100% public APIs)
   - Build time (target: < 5 minutes)

3. **Technical Debt:**
   - Number of code smells
   - Duplicate code percentage
   - Outdated dependencies
   - Security vulnerabilities

## 🛡️ Maintenance Safety Rules

### NEVER:
- Delete files without understanding their purpose
- Move files without updating imports
- Merge code without running tests
- Ignore compiler warnings
- Skip documentation for new features

### ALWAYS:
- Run tests before committing changes
- Update documentation with code changes
- Follow the established directory structure
- Clean up after refactoring
- Maintain backwards compatibility

## 📌 Quick Reference Checklist

Before committing any maintenance changes:

- [ ] All tests pass (`cargo test --all-features`)
- [ ] Code is formatted (`cargo fmt --all`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Documentation is updated
- [ ] Files are in correct directories
- [ ] No duplicate implementations
- [ ] Build artifacts are git-ignored
- [ ] TODO/FIXME items are properly formatted
- [ ] Imports are organized and unused ones removed
- [ ] Error handling is consistent

## 🔄 Continuous Improvement

This maintenance guide should be updated monthly with:
- New patterns discovered
- Lessons learned from issues
- Updated best practices
- Tool improvements
- Process optimizations

Remember: Good maintenance is proactive, not reactive. Regular small improvements prevent large technical debt accumulation.
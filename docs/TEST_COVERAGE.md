# Test Coverage Infrastructure

## Overview

The Neural Trader project uses comprehensive test coverage tools to ensure code quality and maintain an 85% coverage target for new code. We support multiple coverage tools to provide flexibility and detailed insights.

## Coverage Tools

### 1. Cargo Tarpaulin (Primary)

Our primary coverage tool, already included in `Cargo.toml` dev-dependencies.

**Features:**
- Line coverage analysis
- Multiple output formats (HTML, LCOV, JSON)
- Configurable via `tarpaulin.toml`
- Workspace-wide coverage support

### 2. Cargo LLVM-Cov (Alternative)

Source-based code coverage using LLVM instrumentation.

**Features:**
- More accurate branch coverage
- Better support for async code
- Integrated with Rust's built-in coverage
- Lower overhead than ptrace-based tools

## Configuration Files

### `tarpaulin.toml`

Main configuration file for cargo-tarpaulin with profiles for:
- **default**: Full project coverage
- **phase1-neural**: Neural architecture components
- **phase1-features**: Feature engineering modules
- **phase1-backtesting**: Backtesting framework
- **integration**: Integration test coverage
- **unit**: Unit test coverage only

### `.cargo/config.toml`

Cargo configuration with coverage aliases:
- `cargo cov`: Run LLVM coverage
- `cargo cov-html`: Generate HTML report
- `cargo cov-lcov`: Generate LCOV report
- `cargo cov-phase1`: Phase 1 specific coverage

## Coverage Scripts

### 1. `scripts/coverage_all.sh`

Comprehensive coverage analysis for the entire project.

```bash
# Run full coverage
./scripts/coverage_all.sh

# Run with Phase 1 profiles
./scripts/coverage_all.sh --phase1
```

**Output:**
- HTML report: `target/coverage/tarpaulin-report.html`
- LCOV report: `target/coverage/lcov.info`
- JSON report: `target/coverage/tarpaulin-report.json`

### 2. `scripts/coverage_phase1.sh`

Focused coverage analysis for Phase 1 components:
- Neural architecture (predictor, FANN integration)
- Feature engineering (indicators, microstructure, regime detection)
- Backtesting framework (walk-forward, Monte Carlo, A/B testing)

```bash
./scripts/coverage_phase1.sh
```

**Output:**
- Component reports: `target/coverage/phase1/*/tarpaulin-report.html`
- Combined report: `target/coverage/phase1/tarpaulin-report.html`

### 3. `scripts/coverage_llvm.sh`

Alternative coverage using cargo-llvm-cov.

```bash
# Full coverage
./scripts/coverage_llvm.sh

# Phase 1 only
./scripts/coverage_llvm.sh --phase1

# Unit tests only
./scripts/coverage_llvm.sh --unit

# With failure threshold
./scripts/coverage_llvm.sh --fail-under 85
```

## GitHub Actions Integration

Automated coverage runs on:
- Push to main, develop, neural-expand branches
- Pull requests
- Manual workflow dispatch

**Workflow features:**
- Codecov integration
- PR comments with coverage delta
- Coverage artifacts upload
- Phase 1 specific jobs for neural-expand branch

## Coverage Targets

### Overall Target: 85%

We aim for 85% coverage on new code with focus areas:

1. **Neural Components** (Phase 1 Priority)
   - Neural predictor: 85%+
   - FANN integration: 85%+
   - Strategy implementations: 85%+

2. **Feature Engineering**
   - Technical indicators: 85%+
   - Market microstructure: 85%+
   - Regime detection: 80%+

3. **Backtesting Framework**
   - Walk-forward analysis: 85%+
   - Monte Carlo simulation: 85%+
   - A/B testing: 80%+

## Usage Examples

### Quick Coverage Check

```bash
# Using cargo-tarpaulin
cargo tarpaulin --print-summary

# Using cargo-llvm-cov
cargo llvm-cov --summary-only
```

### Detailed HTML Report

```bash
# Generate and open HTML report
./scripts/coverage_all.sh
open target/coverage/tarpaulin-report.html
```

### CI/CD Integration

```yaml
# In GitHub Actions
- name: Run coverage
  run: cargo tarpaulin --out Xml
  
- name: Upload to Codecov
  uses: codecov/codecov-action@v3
```

## Best Practices

1. **Run coverage before commits**: Ensure new code maintains 85% target
2. **Focus on critical paths**: Prioritize coverage for core logic
3. **Use both tools**: Tarpaulin for general use, LLVM-cov for detailed analysis
4. **Monitor trends**: Track coverage over time, not just absolute numbers
5. **Exclude appropriately**: Don't measure generated code or test utilities

## Troubleshooting

### Common Issues

1. **Tarpaulin timeout**: Increase timeout in config or use `--timeout`
2. **LLVM-cov missing**: Install with `cargo install cargo-llvm-cov`
3. **Low coverage**: Check for untested error paths and edge cases
4. **Slow runs**: Use `--test-threads 1` for more stable results

### Coverage Gaps

If coverage is below target:
1. Run `./scripts/coverage_phase1.sh` to identify weak areas
2. Check uncovered lines in HTML reports
3. Add tests for error handling and edge cases
4. Consider using property-based testing for complex logic

## Future Enhancements

- [ ] Integration with mutation testing (cargo-mutants)
- [ ] Coverage trends dashboard
- [ ] Differential coverage for PRs
- [ ] Coverage-guided fuzzing integration
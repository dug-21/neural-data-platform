# Air Quality Feature (air-001) - Complete Index

**Feature ID:** air-001
**Status:** In Development - Phase 1 (Foundation)
**Branch:** feature/air-001-implementation
**Last Updated:** 2025-12-13

---

## Quick Links

| Document | Purpose | Status |
|----------|---------|--------|
| [Specification](./specs/01-specification.md) | Complete requirements and AirGradient integration | ✅ Complete (with corrections) |
| [Architecture](./architecture/01-system-design.md) | System design and Docker architecture | ✅ Complete |
| [Roadmap](./implementation/01-roadmap.md) | 5-phase implementation plan | ✅ Complete |
| [CHANGELOG](./implementation/CHANGELOG.md) | Change tracking | 🔄 Active |
| [Pre-Commit Hooks](./implementation/PRE_COMMIT_HOOKS.md) | Code quality checks | ✅ Complete |
| [Code Review](./README_CODE_REVIEW.md) | MQTT specification corrections | ✅ Complete |

---

## Project Status

### Current Phase: Phase 1 - Foundation
**Progress:** 10% (GitHub integration complete)

### Completed Milestones
- ✅ SPARC Specification (with MQTT corrections identified)
- ✅ SPARC Architecture design
- ✅ Implementation roadmap created
- ✅ Live sensor testing and validation
- ✅ GitHub workflow setup
- ✅ Feature branch created
- ✅ CI/CD pipeline configured

### In Progress
- 🔄 Phase 1.1: Core traits implementation
- 🔄 Phase 1.2: AirGradient domain types
- 🔄 Phase 1.3: Docker development environment

### Upcoming
- ⏳ Phase 1.4: Configuration management (config-store integration)
- ⏳ Phase 2: Data pipeline (MQTT, HTTP, Parquet)
- ⏳ Phase 3: Intelligence layer (AQI, alerts, forecasting)
- ⏳ Phase 4: API & integration
- ⏳ Phase 5: Production deployment

---

## Repository Structure

```
product/features/air-001/
├── INDEX.md                                    # This file - complete navigation
├── README_CODE_REVIEW.md                       # Code review executive summary
├── SPEC_CORRECTIONS_SUMMARY.md                 # Quick reference for spec fixes
├── CODE_REVIEW_SPEC_VS_ACTUAL_MQTT.md         # Detailed MQTT analysis
├── MQTT_FIELD_COMPARISON.md                    # Field availability matrix
├── CORRECTED_SPEC_SECTIONS.md                  # Ready-to-apply spec fixes
│
├── specs/
│   └── 01-specification.md                     # Complete requirements (needs updates)
│
├── architecture/
│   └── 01-system-design.md                     # System architecture
│
├── pseudocode/
│   └── (to be created in Phase 2)
│
├── refinement/
│   └── (to be created in Phase 3)
│
└── implementation/
    ├── 01-roadmap.md                           # 5-phase implementation plan
    ├── CHANGELOG.md                            # Change tracking
    └── PRE_COMMIT_HOOKS.md                     # Code quality documentation
```

---

## GitHub Integration

### Branch Strategy
- **Main Branch:** `main`
- **Feature Branch:** `feature/air-001-implementation`
- **Merge Strategy:** Pull requests with code review

### CI/CD Pipeline
**Location:** `/workspaces/neural-data-platform/.github/workflows/air-001-ci.yml`

**Checks:**
- ✅ Cargo build and compilation check
- ✅ Rustfmt formatting validation
- ✅ Clippy linting (warnings as errors)
- ✅ Test suite execution
- ✅ Code coverage reporting (Codecov)
- ✅ Security audit (cargo-audit)
- ✅ Multi-architecture build preparation (amd64/arm64)
- ✅ TODO/FIXME detection

### Commit Message Template
**Location:** `/workspaces/neural-data-platform/.gitmessage`

**Format:** Conventional Commits with air-001 scopes

**Configure locally:**
```bash
git config commit.template .gitmessage
```

---

## Development Workflow

### 1. Setup Development Environment

```bash
# Clone and checkout feature branch
git checkout feature/air-001-implementation

# Configure commit message template
git config commit.template .gitmessage

# Install pre-commit hooks (optional)
# See product/features/air-001/implementation/PRE_COMMIT_HOOKS.md
```

### 2. Development Cycle

```bash
# Make changes to code
# Run local checks
cargo fmt
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features

# Commit with conventional commit format
git commit
# Template will guide you through the format

# Push to remote
git push origin feature/air-001-implementation
```

### 3. Pull Request Process

1. Ensure all CI checks pass
2. Update CHANGELOG.md with changes
3. Request code review from team
4. Address review feedback
5. Merge when approved

---

## Key Deliverables by Phase

### Phase 1: Foundation (Weeks 1-2)
- [ ] Core traits (TimeSeriesPoint, Store, Source, Forecast)
- [ ] AirGradient domain types (29+ fields)
- [ ] AirGradient parser (MQTT + Local API)
- [ ] Docker development environment
- [ ] Configuration management (config-store)

### Phase 2: Data Pipeline (Weeks 3-4)
- [ ] MQTT ingestion source
- [ ] HTTP polling source
- [ ] Parquet storage implementation
- [ ] Data validation and quality scoring

### Phase 3: Intelligence Layer (Weeks 5-6)
- [ ] EPA AQI calculations
- [ ] Alert engine with thresholds
- [ ] ruv-FANN integration
- [ ] 24-hour forecasting

### Phase 4: API & Integration (Week 7)
- [ ] REST API (Axum)
- [ ] MCP server for Claude integration
- [ ] Health and metrics endpoints
- [ ] OpenAPI documentation

### Phase 5: Production Deployment (Week 8)
- [ ] Multi-architecture Docker images
- [ ] Raspberry Pi 5 deployment
- [ ] Performance testing
- [ ] Release documentation

---

## Critical Issues & Resolutions

### Issue 1: MQTT Topic Pattern Mismatch ✅ IDENTIFIED
**Impact:** Would cause 100% subscription failure
**Resolution:** Documented in CODE_REVIEW_SPEC_VS_ACTUAL_MQTT.md
**Status:** Awaiting specification update

### Issue 2: Field Count Discrepancy ✅ IDENTIFIED
**Impact:** 58% potential data loss (12 fields claimed vs 29 actual)
**Resolution:** Complete field mapping in CORRECTED_SPEC_SECTIONS.md
**Status:** Awaiting specification update

### Issue 3: Field Source Mapping ✅ IDENTIFIED
**Impact:** 55% incorrect field availability claims
**Resolution:** Truth table in MQTT_FIELD_COMPARISON.md
**Status:** Awaiting specification update

---

## Testing Strategy

### Unit Tests
- Target: 90%+ coverage for core traits
- Target: 95%+ coverage for parsers and domain logic
- TDD approach for all new features

### Integration Tests
- MQTT ingestion with testcontainers
- Parquet write/read cycles
- API endpoint testing

### End-to-End Tests
- Full pipeline: MQTT → Parse → Store → Query → API
- Docker container deployment
- Raspberry Pi 5 deployment

### Performance Benchmarks
- Message throughput: 1/sec sustained
- Query latency: <100ms p99 for 1-day range
- Memory usage: <1.5GB on Raspberry Pi 5

---

## Dependencies

### Rust Crates
- `rumqttc` - MQTT client
- `polars` - DataFrame and Parquet
- `axum` - REST API framework
- `tokio` - Async runtime
- `serde` - Serialization
- `chrono` - Timestamp handling

### Workspace Dependencies
- `neural-core` - Core prediction types
- `neural-ml-ops` - ML operations
- `config-store` - Configuration management
- `ruv-fann` - Forecasting models

---

## Success Metrics

| Metric | Target | Current |
|--------|--------|---------|
| Specification Accuracy | 100% | 45% (corrections identified) |
| Code Coverage | >85% | 0% (not yet implemented) |
| CI Pipeline Pass Rate | >95% | 100% (workflow configured) |
| Documentation Completeness | 100% | 80% |
| Performance (Pi 5) | <1.5GB RAM | Not tested |
| MQTT Reliability | 99.9% uptime | Not tested |

---

## Team & Contacts

### Development Team
- **GitHub Integration Agent:** Responsible for Git workflows and CI/CD
- **Core Implementation Team:** TBD
- **Code Review Team:** TBD

### Communication Channels
- **Issues:** GitHub Issues with `air-001` label
- **Pull Requests:** Use PR template (to be created)
- **Documentation:** Update INDEX.md with major changes

---

## Next Steps (Priority Order)

1. ✅ **DONE:** Set up GitHub integration and workflows
2. **NEXT:** Apply specification corrections from code review
3. **NEXT:** Begin Phase 1.1 - Core traits implementation
4. **NEXT:** Create Docker development environment
5. **FUTURE:** Implement MQTT ingestion (Phase 2)

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-13 | Initial index creation |
| 1.1.0 | 2025-12-13 | Added GitHub integration status |

---

## Related Documentation

- [CLAUDE.md](../../../CLAUDE.md) - Project-wide development guidelines
- [GitHub Integration Modes](.claude/agents/github/github-modes.md) - GitHub workflow documentation
- [SPARC Methodology](../../../docs/SPARC.md) - Development methodology

---

**Navigation Tip:** Use this INDEX.md as your starting point for all air-001 feature work. It provides complete navigation and current status.

Last Updated: 2025-12-13 by GitHub Integration Agent

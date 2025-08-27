# src/ Directory Deprecation & Removal Action Plan
*Created: December 27, 2024*

## 🎯 Objective
Complete removal of the legacy src/ directory while maintaining system functionality through the new 3-binary architecture.

## 📊 Current Situation

### The Paradox:
- **90% of src/ is functionally deprecated** (replaced by new services)
- **But contains 27 critical dependencies** (build/test infrastructure)
- **Main blocker**: src/ is still the primary entry point in Cargo.toml

### Validated Findings:
- ✅ New 3-binary architecture is fully self-sufficient
- ✅ EventBus integration eliminated functional dependencies
- ✅ Data-staging service is completely independent
- ❌ Build system still depends on src/
- ❌ Test infrastructure expects src/ structure

## 🚀 4-Week Removal Plan

### Week 1: Infrastructure Decoupling
**Goal**: Remove build system dependencies on src/

#### Day 1-2: Proto Generation Migration
```bash
# 1. Update each service's build.rs to generate protos locally
cd neural-core && mkdir -p src/proto
# Update build.rs to output to src/proto instead of ../src/proto

cd ../neural-ml-ops && mkdir -p src/proto  
# Repeat for each service

# 2. Remove src/proto/*.rs from git
git rm -r src/proto/
echo "src/proto/" >> .gitignore
git commit -m "chore: remove generated proto files from version control"
```

#### Day 3-4: Cargo.toml Migration
```bash
# 1. Create new entry points in V2 services
cp src/main.rs neural-trading/src/bin/neural-trader.rs
cp src/bin/mcp_server.rs neural-core/src/bin/mcp_server.rs

# 2. Update root Cargo.toml
# Change from:
[[bin]]
name = "neural-trader"
path = "src/main.rs"

# To workspace-only configuration:
[workspace]
members = ["neural-core", "neural-ml-ops", "neural-trading", "data-staging"]
```

#### Day 5: Python Test Migration
```bash
# Update all Python imports
find tests -name "*.py" -exec sed -i 's/from src\./from neural_core\./g' {} \;

# Update Python path configurations
find scripts -name "*.py" -exec sed -i 's/sys.path.append.*src.*/# Removed legacy path/g' {} \;
```

### Week 2: Library Migration
**Goal**: Migrate autonomous-platform library functionality

#### Day 1-2: Core Library Migration
```bash
# 1. Extract minimal required exports to neural-core
mkdir -p neural-core/src/platform
cp src/lib.rs neural-core/src/platform/lib.rs

# 2. Update neural-core/Cargo.toml
[lib]
name = "neural_core"
path = "src/lib.rs"

# Add platform module
# In neural-core/src/lib.rs:
pub mod platform;
```

#### Day 3-4: Mass Import Updates
```bash
# Automated import migration
find . -name "*.rs" -type f -exec sed -i \
  's/use autonomous_platform::/use neural_core::platform::/g' {} \;

# Update Cargo.toml dependencies
find . -name "Cargo.toml" -exec sed -i \
  's/autonomous-platform/neural-core/g' {} \;
```

#### Day 5: Validation
```bash
# Ensure everything compiles
cargo clean
cargo build --all --all-targets
cargo test --all
```

### Week 3: Incremental Deletion
**Goal**: Remove deprecated components safely

#### Day 1: Mark & Archive
```bash
# Create backup
tar -czf src_backup_$(date +%Y%m%d).tar.gz src/

# Mark deprecated in code
find src -name "*.rs" -exec sed -i '1i\
#![deprecated(since = "2.0.0", note = "Migrated to 3-binary architecture")]' {} \;
```

#### Day 2-3: Delete Over-Engineered Components
```bash
# Remove confirmed over-engineered systems
rm -rf src/utils/market_hours/      # 2,400 lines → 50 lines in config
rm -rf src/config/sector_models.rs  # 2,000 lines → 50 lines in features
rm -rf src/backtesting/             # Not needed for live trading
```

#### Day 4-5: Delete Migrated Components
```bash
# Remove fully migrated directories
rm -rf src/neural/          # → neural-ml-ops
rm -rf src/features/        # → neural-ml-ops/features  
rm -rf src/action_layer/    # → neural-trading
rm -rf src/daa/            # → neural-trading/daa
rm -rf src/templates/       # No longer needed
rm -rf src/examples/        # Outdated

# Test after each deletion
cargo test --all
```

### Week 4: Final Cleanup
**Goal**: Complete src/ removal and validate system

#### Day 1-2: Final Migration
```bash
# Migrate remaining useful components
mv src/mcp/* neural-core/src/mcp/

# Update any remaining references
grep -r "src/" --include="*.rs" --include="*.toml" --include="*.py"
```

#### Day 3: Complete Removal
```bash
# Final checks
cargo build --all --release
cargo test --all

# If all tests pass:
rm -rf src/
git add -A
git commit -m "feat: complete migration from monolithic src/ to 3-binary architecture"
```

#### Day 4-5: CI/CD Updates
```yaml
# Update .github/workflows/*.yml
# Remove:
paths:
  - 'src/**'
  
# Update build commands
# From: cargo build --bin neural-trader
# To: cargo build --all
```

## 🛡️ Risk Mitigation

### Rollback Points:
1. **After each phase**: Tag git for easy rollback
2. **Backup archive**: Keep src_backup.tar.gz for 30 days
3. **Feature flags**: Add migration flags to switch between old/new

### Testing Strategy:
```bash
# Run after EVERY change:
./scripts/validate_migration.sh

# Contents:
#!/bin/bash
cargo build --all || exit 1
cargo test --all || exit 1
cargo clippy --all || exit 1
python -m pytest tests/ || exit 1
```

### Monitoring:
- Track build times before/after
- Monitor test coverage
- Check for import errors
- Validate proto generation

## ✅ Success Criteria

### Must Have:
- [ ] All services compile without src/
- [ ] All tests pass without src/
- [ ] CI/CD pipelines work without src/
- [ ] Proto generation works independently
- [ ] MCP integration maintained

### Nice to Have:
- [ ] Reduced build times
- [ ] Cleaner workspace structure
- [ ] Simplified debugging
- [ ] Better code organization

## 📈 Expected Outcomes

### Immediate Benefits:
- **50% code reduction** (45k → 22.5k lines)
- **Clear service boundaries**
- **No monolithic dependencies**
- **Faster incremental builds**

### Long-term Benefits:
- **Independent service deployment**
- **Easier onboarding** (clear structure)
- **Reduced maintenance burden**
- **Better testability**

## 🚦 Go/No-Go Decision Points

### Week 1 Checkpoint:
- Proto generation working? ✅/❌
- Cargo.toml updated? ✅/❌
- Python tests passing? ✅/❌
→ **If any ❌**: Pause and fix before proceeding

### Week 2 Checkpoint:
- Library migration complete? ✅/❌
- All imports updated? ✅/❌
- Full test suite passing? ✅/❌
→ **If any ❌**: Rollback and reassess

### Week 3 Checkpoint:
- Deprecated code removed? ✅/❌
- System still functional? ✅/❌
- No regression in tests? ✅/❌
→ **If any ❌**: Restore from backup

### Final Checkpoint:
- src/ fully removed? ✅/❌
- All services operational? ✅/❌
- CI/CD updated? ✅/❌
→ **If all ✅**: Migration complete! 🎉

## 📝 Notes

- **DO NOT RUSH**: Each phase builds on the previous
- **TEST EVERYTHING**: Better safe than broken
- **DOCUMENT ISSUES**: Keep migration log for future reference
- **COORDINATE TEAM**: Ensure everyone knows the plan

---
*This action plan should be executed with careful attention to each phase's success criteria.*
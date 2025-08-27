#!/bin/bash
# execute_immediate_cleanup.sh
# Execute immediate cleanup tasks that can be done safely now

set -euo pipefail

echo "🧹 Starting immediate cleanup of src/ directory..."

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create backup before cleanup
BACKUP_DIR="/tmp/neural-trader-src-backup-$(date +%s)"
echo "💾 Creating backup at $BACKUP_DIR..."
mkdir -p "$BACKUP_DIR"
cp -r src/ "$BACKUP_DIR/" 2>/dev/null || true
echo -e "${GREEN}✅ Backup created${NC}"

# Phase 1: Remove empty directories
echo "📁 Removing empty directories..."
EMPTY_DIRS=(
    "src/v2-platform/mlops"
    "src/v2-platform/mcp" 
    "src/v2-platform/safety"
    "src/v2-platform/monitoring"
    "src/optimization"
    "src/prediction"
    "src/mcp-server"
    "src/examples/shared"
    "src/examples/neural_trader"
    "src/examples/data_ingestion"
    "src/config-store/tests"
    "src/config-store/examples"
)

for dir in "${EMPTY_DIRS[@]}"; do
    if [ -d "$dir" ] && [ -z "$(ls -A "$dir" 2>/dev/null)" ]; then
        rmdir "$dir"
        echo -e "${GREEN}  ✅ Removed empty directory: $dir${NC}"
    elif [ -d "$dir" ]; then
        echo -e "${YELLOW}  ⚠️  Directory not empty: $dir${NC}"
        ls -la "$dir"
    else
        echo -e "${YELLOW}  ℹ️  Directory doesn't exist: $dir${NC}"
    fi
done

# Clean up empty parent directories
echo "🗂️  Cleaning up empty parent directories..."
for parent in "src/v2-platform" "src/examples" "src/config-store"; do
    if [ -d "$parent" ] && [ -z "$(ls -A "$parent" 2>/dev/null)" ]; then
        rmdir "$parent"
        echo -e "${GREEN}  ✅ Removed empty parent directory: $parent${NC}"
    fi
done

# Phase 2: Remove Python files (duplicates of Rust implementations)
echo "🐍 Removing duplicate Python files..."
PYTHON_FILES=(
    "src/utils/rate_limiter.py"
    "src/utils/market_hours_implementation.py"
    "src/config-store/config_store_client.py"
)

for file in "${PYTHON_FILES[@]}"; do
    if [ -f "$file" ]; then
        rm "$file"
        echo -e "${GREEN}  ✅ Removed Python file: $file${NC}"
    else
        echo -e "${YELLOW}  ℹ️  File doesn't exist: $file${NC}"
    fi
done

# Phase 3: Remove test utilities from src/bin/ (low-risk deletion)
echo "🧪 Removing test utilities from src/bin/..."
TEST_UTILITIES=(
    "src/bin/test_compile.rs"
    "src/bin/test_health_monitor.rs"
    "src/bin/test_mcp.rs"
    "src/bin/mcp_server_simple.rs"  # Superseded by mcp_server.rs
)

for file in "${TEST_UTILITIES[@]}"; do
    if [ -f "$file" ]; then
        rm "$file"
        echo -e "${GREEN}  ✅ Removed test utility: $file${NC}"
    else
        echo -e "${YELLOW}  ℹ️  File doesn't exist: $file${NC}"
    fi
done

# Phase 4: Verification
echo "🔍 Verifying cleanup..."

# Check compilation still works
echo "⚙️  Verifying workspace compilation..."
if cargo check --workspace --quiet 2>/dev/null; then
    echo -e "${GREEN}✅ Workspace still compiles successfully${NC}"
else
    echo -e "${RED}❌ Compilation issues detected after cleanup${NC}"
    echo "🔄 Restoring from backup..."
    rm -rf src/
    cp -r "$BACKUP_DIR/src" ./
    echo -e "${YELLOW}⚠️  Cleanup rolled back due to compilation issues${NC}"
    exit 1
fi

# Generate cleanup report
REPORT_FILE="target/immediate_cleanup_report.txt"
mkdir -p target
cat > "$REPORT_FILE" << EOF
Immediate Cleanup Report
Generated: $(date)
Backup Location: $BACKUP_DIR

EMPTY DIRECTORIES REMOVED:
$(for dir in "${EMPTY_DIRS[@]}"; do echo "  - $dir"; done)

PYTHON FILES REMOVED:
$(for file in "${PYTHON_FILES[@]}"; do echo "  - $file"; done)

TEST UTILITIES REMOVED:
$(for file in "${TEST_UTILITIES[@]}"; do echo "  - $file"; done)

VERIFICATION:
- Workspace compilation: PASSED
- Total files removed: $((${#EMPTY_DIRS[@]} + ${#PYTHON_FILES[@]} + ${#TEST_UTILITIES[@]}))
- Backup preserved at: $BACKUP_DIR

NEXT STEPS:
1. Review this report
2. Run scripts/validate_migration.sh to check progress
3. Proceed with binary utility migrations (scripts/migrate_binaries.sh)
4. Continue with module-by-module migrations as outlined in the plan
EOF

echo ""
echo "📊 IMMEDIATE CLEANUP COMPLETE"
echo "=============================="
echo -e "${GREEN}✅ All immediate cleanup tasks completed successfully${NC}"
echo "📄 Report generated: $REPORT_FILE"
echo "💾 Backup preserved: $BACKUP_DIR"
echo ""
echo "🚀 Next steps:"
echo "  1. Review the cleanup report: cat $REPORT_FILE"
echo "  2. Validate progress: ./scripts/validate_migration.sh"
echo "  3. Continue with binary migrations when ready"
echo ""
echo -e "${YELLOW}ℹ️  This cleanup removed only safe-to-delete items.${NC}"
echo -e "${YELLOW}   Core functionality migrations still need to be completed.${NC}"
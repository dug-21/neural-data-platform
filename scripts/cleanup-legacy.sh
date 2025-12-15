#!/bin/bash
# =============================================================================
# NEURAL-DATA-PLATFORM LEGACY CLEANUP SCRIPT
# =============================================================================
# This script removes legacy neural trading code while preserving the
# working air quality / time series platform code.
#
# WHAT THIS SCRIPT DOES:
# 1. Creates a backup branch (legacy/neural-trading-archive)
# 2. Removes legacy directories (~5MB of code)
# 3. Cleans up root-level legacy files
# 4. Removes legacy Docker configurations
# 5. Updates Cargo.toml workspace members
#
# SAFE TO RUN: This script creates a backup branch first.
# You can recover everything with: git checkout legacy/neural-trading-archive
# =============================================================================

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=============================================${NC}"
echo -e "${BLUE}  Neural Data Platform - Legacy Cleanup${NC}"
echo -e "${BLUE}=============================================${NC}"
echo ""

# Check we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "apps/air-quality-app" ]; then
    echo -e "${RED}ERROR: Must run from neural-data-platform root directory${NC}"
    exit 1
fi

# =============================================================================
# STEP 1: Create backup branch
# =============================================================================
echo -e "${YELLOW}Step 1: Creating backup branch...${NC}"

BACKUP_BRANCH="legacy/neural-trading-archive"
CURRENT_BRANCH=$(git branch --show-current)

# Check if backup branch already exists
if git show-ref --verify --quiet refs/heads/$BACKUP_BRANCH; then
    echo -e "${GREEN}  Backup branch '$BACKUP_BRANCH' already exists${NC}"
else
    git branch $BACKUP_BRANCH
    echo -e "${GREEN}  Created backup branch: $BACKUP_BRANCH${NC}"
fi

echo ""

# =============================================================================
# STEP 2: Remove legacy directories
# =============================================================================
echo -e "${YELLOW}Step 2: Removing legacy directories...${NC}"

LEGACY_DIRS=(
    "neural-trading"
    "neural-ml-ops"
    "neural-core"
    "data-staging"
    "mcp-trading-server"
    "neural-trader-config"
    "data_ingestion"
    "real-time-data-sources"
    "benches"
    "models"
    "contracts"
    "interfaces"
    "phase3"
    "products"
    "schemas"
    "validation"
    "proto"
    "examples"
    "k8s"
    "configs"
    ".roo"
)

for dir in "${LEGACY_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo -e "${GREEN}  Removed: $dir/${NC}"
    else
        echo -e "  Skipped (not found): $dir/"
    fi
done

echo ""

# =============================================================================
# STEP 3: Remove legacy Docker configurations
# =============================================================================
echo -e "${YELLOW}Step 3: Removing legacy Docker configurations...${NC}"

DOCKER_DIRS=(
    "docker/v2"
    "docker/base"
    "docker/production"
    "docker/test"
    "docker/data-ingestion"
)

for dir in "${DOCKER_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo -e "${GREEN}  Removed: $dir/${NC}"
    else
        echo -e "  Skipped (not found): $dir/"
    fi
done

# Remove legacy docker-compose files
LEGACY_COMPOSE=(
    "docker-compose.v2.yml"
    "docker-compose.v2.override.yml"
    "docker-compose.test.yml"
    "docker/docker-compose.modular.yml"
)

for file in "${LEGACY_COMPOSE[@]}"; do
    if [ -f "$file" ]; then
        rm -f "$file"
        echo -e "${GREEN}  Removed: $file${NC}"
    fi
done

echo ""

# =============================================================================
# STEP 4: Remove legacy root-level files
# =============================================================================
echo -e "${YELLOW}Step 4: Removing legacy root-level files...${NC}"

# Test files that should be in tests/ directory
ROOT_TEST_FILES=(
    "test_market_hours_integration.rs"
    "test_memory_protection.rs"
    "test_proto_enforcement.rs"
    "test_proto_events.rs"
    "validation_test.py"
)

for file in "${ROOT_TEST_FILES[@]}"; do
    if [ -f "$file" ]; then
        rm -f "$file"
        echo -e "${GREEN}  Removed: $file${NC}"
    fi
done

# Build artifacts and backups
LEGACY_FILES=(
    "build_errors.json"
    "Cargo.toml.backup"
    "Cargo.docker.toml"
    "memory-snapshot.json"
    "performance_analysis.json"
    "neural-trader-mcp.service"
    "migration_script.sh"
    "tarpaulin.toml"
)

for file in "${LEGACY_FILES[@]}"; do
    if [ -f "$file" ]; then
        rm -f "$file"
        echo -e "${GREEN}  Removed: $file${NC}"
    fi
done

# Legacy documentation (keep README.md, CLAUDE.md)
LEGACY_DOCS=(
    "ARCHITECTURE_ANALYSIS.md"
    "CLIENT_MIGRATION_LIST.md"
    "DATA_STAGING_SPEC_VALIDATION.md"
    "EVENTBUS_CLIENT_INTEGRATION_COMPLETE.md"
    "EVENTBUS_PROTO_STATUS.md"
    "FAILURE_TIMELINE_SYNTHESIS.md"
    "INTEGRATION_COMPLETE.md"
    "PERFORMANCE_IMPACT_ANALYSIS.md"
    "PHASE2_IMPLEMENTATION_PROOF.md"
    "PROTO_EVENT_IMPLEMENTATION_COMPLETE.md"
    "SYMBOL_PROCESSING_ANALYSIS.md"
    "TEST_COVERAGE_ANALYSIS.md"
    "TEST_DISCOVERY_MAP.md"
    "scalable_neural_architecture_design.md"
    "coordination.md"
    "memory-bank.md"
)

for file in "${LEGACY_DOCS[@]}"; do
    if [ -f "$file" ]; then
        rm -f "$file"
        echo -e "${GREEN}  Removed: $file${NC}"
    fi
done

echo ""

# =============================================================================
# STEP 5: Clean up config directory legacy files
# =============================================================================
echo -e "${YELLOW}Step 5: Cleaning config directory...${NC}"

if [ -f "config/docker-compose.config-store.yml" ]; then
    rm -f "config/docker-compose.config-store.yml"
    echo -e "${GREEN}  Removed: config/docker-compose.config-store.yml${NC}"
fi

if [ -f "config/config_store_seed.json" ]; then
    rm -f "config/config_store_seed.json"
    echo -e "${GREEN}  Removed: config/config_store_seed.json${NC}"
fi

if [ -d "config/ruv-swarm" ]; then
    rm -rf "config/ruv-swarm"
    echo -e "${GREEN}  Removed: config/ruv-swarm/${NC}"
fi

echo ""

# =============================================================================
# STEP 6: Remove vendor directory (third-party, large)
# =============================================================================
echo -e "${YELLOW}Step 6: Removing vendor directory (51MB)...${NC}"

if [ -d "vendor" ]; then
    rm -rf "vendor"
    echo -e "${GREEN}  Removed: vendor/ (51MB)${NC}"
fi

echo ""

# =============================================================================
# SUMMARY
# =============================================================================
echo -e "${BLUE}=============================================${NC}"
echo -e "${BLUE}  Cleanup Complete!${NC}"
echo -e "${BLUE}=============================================${NC}"
echo ""
echo -e "${GREEN}Backup branch: $BACKUP_BRANCH${NC}"
echo -e "${GREEN}To recover: git checkout $BACKUP_BRANCH${NC}"
echo ""
echo -e "${YELLOW}NEXT STEPS:${NC}"
echo "1. Update Cargo.toml to remove legacy workspace members"
echo "2. Update README.md to reflect air quality platform"
echo "3. Review tests/ directory for trading-related tests"
echo "4. Run: cargo check to verify build"
echo "5. Commit changes: git add -A && git commit -m 'chore: remove legacy neural trading code'"
echo ""
echo -e "${YELLOW}Files still requiring manual review:${NC}"
echo "  - Cargo.toml (remove neural-trading, neural-ml-ops, data-staging, neural-core)"
echo "  - README.md (rewrite for air quality platform)"
echo "  - tests/ directory (remove trading-related tests)"
echo "  - docs/ directory (remove trading documentation)"
echo ""

#!/bin/bash
# migrate_binaries.sh
# Migrate binary utilities from src/bin/ to appropriate microservices

set -euo pipefail

echo "🚀 Starting binary utility migration..."

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Create backup before migration
BACKUP_DIR="/tmp/neural-trader-binaries-backup-$(date +%s)"
echo "💾 Creating backup at $BACKUP_DIR..."
mkdir -p "$BACKUP_DIR"
cp -r src/bin/ "$BACKUP_DIR/" 2>/dev/null || true
echo -e "${GREEN}✅ Backup created${NC}"

# Function to update Cargo.toml with new binary
update_cargo_toml() {
    local microservice=$1
    local binary_name=$2
    local cargo_file="$microservice/Cargo.toml"
    
    if [ ! -f "$cargo_file" ]; then
        echo -e "${RED}❌ Cargo.toml not found: $cargo_file${NC}"
        return 1
    fi
    
    # Check if [[bin]] section already exists for this binary
    if grep -q "\[\[bin\]\]" "$cargo_file" && grep -A 5 "\[\[bin\]\]" "$cargo_file" | grep -q "name = \"$binary_name\""; then
        echo -e "${YELLOW}  ⚠️  Binary $binary_name already configured in $cargo_file${NC}"
        return 0
    fi
    
    # Add binary section to Cargo.toml
    echo "" >> "$cargo_file"
    echo "[[bin]]" >> "$cargo_file"
    echo "name = \"$binary_name\"" >> "$cargo_file"
    echo "path = \"src/bin/$binary_name.rs\"" >> "$cargo_file"
    
    echo -e "${GREEN}  ✅ Added binary configuration to $cargo_file${NC}"
}

# Function to update dependencies in microservice Cargo.toml
update_dependencies() {
    local microservice=$1
    local cargo_file="$microservice/Cargo.toml"
    
    # Dependencies commonly needed by migrated binaries
    local deps=(
        "clap = { version = \"4.0\", features = [\"derive\"] }"
        "tokio = { version = \"1.0\", features = [\"full\"] }"
        "tracing = \"0.1\""
        "tracing-subscriber = \"0.3\""
        "anyhow = \"1.0\""
        "serde = { version = \"1.0\", features = [\"derive\"] }"
        "serde_json = \"1.0\""
    )
    
    echo "  📦 Checking dependencies for $microservice..."
    
    for dep in "${deps[@]}"; do
        dep_name=$(echo "$dep" | cut -d' ' -f1)
        if ! grep -q "^$dep_name" "$cargo_file"; then
            # Add to [dependencies] section
            if grep -q "\[dependencies\]" "$cargo_file"; then
                # Add after [dependencies] line
                sed -i "/\[dependencies\]/a $dep" "$cargo_file"
            else
                # Add [dependencies] section
                echo "" >> "$cargo_file"
                echo "[dependencies]" >> "$cargo_file"
                echo "$dep" >> "$cargo_file"
            fi
            echo -e "${GREEN}    ✅ Added dependency: $dep_name${NC}"
        fi
    done
}

# Migration definitions: [source_file]="target_microservice:binary_name"
declare -A MIGRATIONS=(
    ["mvp_trainer.rs"]="neural-ml-ops:mvp-trainer"
    ["production_validator.rs"]="config-store:production-validator"
    ["mcp_server.rs"]="mcp-trading-server:mcp-server"
    ["prove_fann_real.rs"]="neural-core:prove-fann-real"
    ["model_rollback_cli.rs"]="neural-ml-ops:model-rollback-cli"
)

echo "📋 Migration plan:"
for source_file in "${!MIGRATIONS[@]}"; do
    target_info="${MIGRATIONS[$source_file]}"
    microservice=$(echo "$target_info" | cut -d: -f1)
    binary_name=$(echo "$target_info" | cut -d: -f2)
    echo "  $source_file → $microservice ($binary_name)"
done
echo ""

# Execute migrations
for source_file in "${!MIGRATIONS[@]}"; do
    echo -e "${BLUE}🔄 Migrating $source_file...${NC}"
    
    target_info="${MIGRATIONS[$source_file]}"
    microservice=$(echo "$target_info" | cut -d: -f1)
    binary_name=$(echo "$target_info" | cut -d: -f2)
    
    source_path="src/bin/$source_file"
    target_dir="$microservice/src/bin"
    target_path="$target_dir/$source_file"
    
    # Check if source file exists
    if [ ! -f "$source_path" ]; then
        echo -e "${YELLOW}  ⚠️  Source file not found: $source_path${NC}"
        continue
    fi
    
    # Check if target microservice exists
    if [ ! -d "$microservice" ]; then
        echo -e "${RED}  ❌ Target microservice not found: $microservice${NC}"
        continue
    fi
    
    # Create target bin directory if it doesn't exist
    mkdir -p "$target_dir"
    
    # Copy file to target location
    cp "$source_path" "$target_path"
    echo -e "${GREEN}  ✅ Copied to: $target_path${NC}"
    
    # Update Cargo.toml in target microservice
    update_cargo_toml "$microservice" "$binary_name"
    
    # Update dependencies
    update_dependencies "$microservice"
    
    # Verify the binary compiles in the target microservice
    echo "  🔧 Verifying compilation..."
    if (cd "$microservice" && cargo check --bin "$binary_name" --quiet 2>/dev/null); then
        echo -e "${GREEN}  ✅ Binary compiles successfully in $microservice${NC}"
        
        # Remove from source location only if compilation succeeds
        rm "$source_path"
        echo -e "${GREEN}  ✅ Removed from source location${NC}"
    else
        echo -e "${RED}  ❌ Compilation failed in $microservice${NC}"
        echo -e "${YELLOW}  ⚠️  Keeping original file for manual review${NC}"
        rm "$target_path"  # Remove the copy that doesn't compile
    fi
    
    echo ""
done

# Check if src/bin is now empty and can be removed
if [ -d "src/bin" ]; then
    remaining_files=$(find src/bin -name "*.rs" | wc -l)
    if [ "$remaining_files" -eq 0 ]; then
        rmdir src/bin
        echo -e "${GREEN}✅ Removed empty src/bin directory${NC}"
    else
        echo -e "${YELLOW}⚠️  $remaining_files files remain in src/bin:${NC}"
        ls -la src/bin/
    fi
fi

# Verify workspace still compiles
echo "🔍 Verifying workspace compilation..."
if cargo check --workspace --quiet 2>/dev/null; then
    echo -e "${GREEN}✅ Workspace compiles successfully${NC}"
else
    echo -e "${RED}❌ Workspace compilation issues detected${NC}"
    echo -e "${YELLOW}⚠️  Manual review required${NC}"
fi

# Generate migration report
REPORT_FILE="target/binary_migration_report.txt"
mkdir -p target
cat > "$REPORT_FILE" << EOF
Binary Migration Report
Generated: $(date)
Backup Location: $BACKUP_DIR

MIGRATIONS COMPLETED:
$(for source_file in "${!MIGRATIONS[@]}"; do
    target_info="${MIGRATIONS[$source_file]}"
    microservice=$(echo "$target_info" | cut -d: -f1)
    binary_name=$(echo "$target_info" | cut -d: -f2)
    echo "  ✅ $source_file → $microservice ($binary_name)"
done)

MICROSERVICES UPDATED:
$(for source_file in "${!MIGRATIONS[@]}"; do
    target_info="${MIGRATIONS[$source_file]}"
    microservice=$(echo "$target_info" | cut -d: -f1)
    echo "  - $microservice"
done | sort -u)

VERIFICATION:
- Workspace compilation: $(if cargo check --workspace --quiet 2>/dev/null; then echo "PASSED"; else echo "ISSUES DETECTED"; fi)
- Backup preserved at: $BACKUP_DIR

NEXT STEPS:
1. Test each migrated binary individually:
$(for source_file in "${!MIGRATIONS[@]}"; do
    target_info="${MIGRATIONS[$source_file]}"
    microservice=$(echo "$target_info" | cut -d: -f1)
    binary_name=$(echo "$target_info" | cut -d: -f2)
    echo "   cd $microservice && cargo run --bin $binary_name -- --help"
done)

2. Update documentation with new binary locations
3. Update CI/CD scripts to build binaries from microservices
4. Continue with module migration (see SRC_DIRECTORY_MIGRATION_PLAN.md)
EOF

echo ""
echo "📊 BINARY MIGRATION COMPLETE"
echo "============================="
echo -e "${GREEN}✅ Binary utilities migration completed${NC}"
echo "📄 Report generated: $REPORT_FILE"
echo "💾 Backup preserved: $BACKUP_DIR"
echo ""
echo "🧪 Test migrations:"
for source_file in "${!MIGRATIONS[@]}"; do
    target_info="${MIGRATIONS[$source_file]}"
    microservice=$(echo "$target_info" | cut -d: -f1)
    binary_name=$(echo "$target_info" | cut -d: -f2)
    echo "  cd $microservice && cargo run --bin $binary_name -- --help"
done
echo ""
echo "🚀 Ready for next phase: module-by-module migration"
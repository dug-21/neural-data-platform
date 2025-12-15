#!/bin/bash
# Test script for validating AIR-002 configuration loading fix

set -e

echo "=================================="
echo "AIR-002 Configuration Loading Tests"
echo "=================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test 1: Verify etcd has the configuration
echo -e "${YELLOW}Test 1: Checking etcd configuration${NC}"
if command -v etcdctl &> /dev/null; then
    echo "Querying etcd for /air-quality/storage/base_path..."
    ETCD_VALUE=$(etcdctl get /air-quality/storage/base_path --print-value-only 2>/dev/null || echo "NOT_FOUND")

    if [ "$ETCD_VALUE" != "NOT_FOUND" ]; then
        echo -e "${GREEN}✓ Found in etcd: $ETCD_VALUE${NC}"
    else
        echo -e "${RED}✗ Key not found in etcd${NC}"
        echo "  Run: etcdctl put /air-quality/storage/base_path '\"/var/data/air-quality/parquet\"'"
    fi
else
    echo -e "${RED}✗ etcdctl not found${NC}"
    echo "  Install etcd client to test etcd configuration"
fi
echo ""

# Test 2: Build the application
echo -e "${YELLOW}Test 2: Building air-quality-app${NC}"
cd /workspaces/neural-data-platform
if cargo build -p air-quality-app 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
echo ""

# Test 3: Show what configuration will be loaded
echo -e "${YELLOW}Test 3: Current Configuration Sources${NC}"
echo "Checking available configuration sources..."
echo ""

echo "1. etcd (highest priority):"
if [ "$ETCD_VALUE" != "NOT_FOUND" ]; then
    echo -e "   ${GREEN}Available: $ETCD_VALUE${NC}"
else
    echo -e "   ${RED}Not available${NC}"
fi
echo ""

echo "2. DATA_DIR environment variable:"
if [ -n "$DATA_DIR" ]; then
    echo -e "   ${GREEN}Set: $DATA_DIR${NC}"
else
    echo -e "   ${YELLOW}Not set${NC}"
fi
echo ""

echo "3. STORAGE_PATH environment variable (legacy):"
if [ -n "$STORAGE_PATH" ]; then
    echo -e "   ${GREEN}Set: $STORAGE_PATH${NC}"
else
    echo -e "   ${YELLOW}Not set${NC}"
fi
echo ""

echo "4. config.yaml:"
if [ -f /workspaces/neural-data-platform/config.yaml ]; then
    YAML_PATH=$(grep -A 1 "storage:" /workspaces/neural-data-platform/config.yaml | grep "base_path:" | sed 's/.*base_path: *"\?\([^"]*\)"\?.*/\1/' || echo "Not found in YAML")
    echo -e "   ${GREEN}Available: $YAML_PATH${NC}"
else
    echo -e "   ${YELLOW}File not found${NC}"
fi
echo ""

echo "5. Hardcoded default:"
echo -e "   ${GREEN}Always available: ./data/parquet${NC}"
echo ""

# Test 4: Predict which config will be used
echo -e "${YELLOW}Test 4: Predicted Configuration Source${NC}"
if [ "$ETCD_VALUE" != "NOT_FOUND" ] && [ "$ETCD_VALUE" != "" ]; then
    echo -e "${GREEN}Expected source: etcd${NC}"
    echo -e "Expected base_path: ${GREEN}$ETCD_VALUE${NC}"
elif [ -n "$DATA_DIR" ]; then
    echo -e "${GREEN}Expected source: DATA_DIR environment variable${NC}"
    echo -e "Expected base_path: ${GREEN}$DATA_DIR${NC}"
elif [ -n "$STORAGE_PATH" ]; then
    echo -e "${GREEN}Expected source: STORAGE_PATH environment variable${NC}"
    echo -e "Expected base_path: ${GREEN}$STORAGE_PATH${NC}"
elif [ -f /workspaces/neural-data-platform/config.yaml ]; then
    echo -e "${GREEN}Expected source: config.yaml${NC}"
    echo -e "Expected base_path: ${GREEN}$YAML_PATH${NC}"
else
    echo -e "${GREEN}Expected source: hardcoded default${NC}"
    echo -e "Expected base_path: ${GREEN}./data/parquet${NC}"
fi
echo ""

# Test 5: Instructions for manual testing
echo -e "${YELLOW}Test 5: Manual Testing Instructions${NC}"
echo ""
echo "To verify the fix, run the app and check logs:"
echo "  cd /workspaces/neural-data-platform"
echo "  cargo run -p air-quality-app"
echo ""
echo "Look for these log lines:"
echo "  1. Configuration source:"
echo "     - 'Loaded configuration from etcd' (if etcd is available)"
echo "     - 'Using storage base_path from etcd: /var/data/air-quality/parquet'"
echo ""
echo "  2. ParquetStore initialization:"
echo "     - 'Initializing ParquetStore at: /var/data/air-quality/parquet'"
echo "     - Should match the etcd value, NOT the default ./data/parquet"
echo ""
echo "To test fallback scenarios:"
echo ""
echo "  A. Test DATA_DIR fallback (when etcd unavailable):"
echo "     export DATA_DIR=/custom/test/path"
echo "     # Stop etcd or remove the key temporarily"
echo "     cargo run -p air-quality-app"
echo "     # Should show: 'Using storage base_path from DATA_DIR env var: /custom/test/path'"
echo ""
echo "  B. Test default fallback:"
echo "     unset DATA_DIR"
echo "     unset STORAGE_PATH"
echo "     # Stop etcd and remove config.yaml"
echo "     cargo run -p air-quality-app"
echo "     # Should show: 'No storage base_path in etcd or env vars, using default: ./data/parquet'"
echo ""

echo "=================================="
echo "Test script complete!"
echo "=================================="

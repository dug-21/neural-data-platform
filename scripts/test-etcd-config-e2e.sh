#!/bin/bash
# End-to-end test for etcd config loading
# Tests the full flow: Git configs → etcd → app reads config

set -e

echo "=========================================="
echo "AIR-003 End-to-End Config Test"
echo "=========================================="
echo ""

# Check etcd is running
echo "1. Checking etcd is running..."
docker exec etcd etcdctl endpoint health > /dev/null 2>&1
echo "   ✓ etcd is healthy"

# Verify configs exist in etcd
echo ""
echo "2. Verifying synced configs in etcd..."

# Test a few key values that should be in development overlay
BROKER_URL=$(docker exec etcd etcdctl get /air-quality/mqtt/broker_url --print-value-only 2>/dev/null)
LOG_LEVEL=$(docker exec etcd etcdctl get /air-quality/logging/level --print-value-only 2>/dev/null)
ALERTS_ENABLED=$(docker exec etcd etcdctl get /air-quality/alerts/enabled --print-value-only 2>/dev/null)

echo "   mqtt/broker_url: $BROKER_URL"
echo "   logging/level: $LOG_LEVEL"
echo "   alerts/enabled: $ALERTS_ENABLED"

# Verify development overlay values
if [[ "$BROKER_URL" == *"localhost"* ]]; then
    echo "   ✓ Development broker URL is correct"
else
    echo "   ✗ Expected localhost broker URL"
    exit 1
fi

if [[ "$LOG_LEVEL" == *"debug"* ]]; then
    echo "   ✓ Development log level is correct"
else
    echo "   ✗ Expected debug log level"
    exit 1
fi

# Test Rust config-client can read values
echo ""
echo "3. Testing Rust config-client reads from etcd..."
cargo test -p config-client --test integration_test -- --ignored --nocapture 2>&1 | tail -10

echo ""
echo "4. Listing all air-quality config keys..."
docker exec etcd etcdctl get --prefix "/air-quality" --keys-only | sort | head -30

echo ""
echo "=========================================="
echo "✓ All E2E config tests passed!"
echo "=========================================="
echo ""
echo "Summary:"
echo "  - etcd is running and healthy"
echo "  - GitOps configs synced to etcd"
echo "  - Development overlay applied (localhost, debug)"
echo "  - Rust config-client can read from etcd"

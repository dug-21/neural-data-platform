#!/bin/bash
# Test Redis connection after devcontainer rebuild

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

log_info "Testing Redis connectivity..."

# Test via hostname (should work after network config)
log_info "Testing hostname resolution..."
if redis-cli -h redis ping 2>/dev/null; then
    log_info "✓ Redis hostname resolves and responds"
else
    log_error "✗ Cannot connect via hostname 'redis'"
fi

# Test via environment variable
log_info "Testing REDIS_URL environment variable..."
if [ -n "$REDIS_URL" ]; then
    log_info "REDIS_URL is set: $REDIS_URL"
    # Extract host from URL
    REDIS_HOST=$(echo $REDIS_URL | sed -n 's|redis://\([^:]*\).*|\1|p')
    if redis-cli -h $REDIS_HOST ping 2>/dev/null; then
        log_info "✓ Can connect using REDIS_URL host"
    else
        log_error "✗ Cannot connect to $REDIS_HOST from REDIS_URL"
    fi
else
    log_warn "REDIS_URL not set"
fi

# Test from Rust
log_info "Testing Redis from Rust..."
cd /workspaces/neural-trader/config-store
if cargo test test_redis_store_creation -- --nocapture 2>&1 | grep -q "test.*passed"; then
    log_info "✓ Rust Redis test passed"
else
    log_warn "✗ Rust Redis test failed (may need network rebuild)"
fi

log_info "Network diagnostics:"
echo "Current IP: $(hostname -I)"
echo "Redis container network:"
docker inspect neural-redis --format='{{range .NetworkSettings.Networks}}{{.NetworkName}}: {{.IPAddress}}{{end}}' 2>/dev/null || echo "Cannot inspect Redis container"
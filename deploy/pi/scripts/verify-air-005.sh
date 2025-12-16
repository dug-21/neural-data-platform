#!/bin/bash
# AIR-005 Deployment Verification Script
# Verifies that OpenWeatherMap integration is working correctly
#
# USAGE:
#   ./scripts/verify-air-005.sh
#
# EXIT CODES:
#   0 - All checks passed
#   1 - One or more checks failed

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track overall success
ALL_PASSED=true

# Helper functions
check_pass() {
    echo -e "${GREEN}✅ $1${NC}"
}

check_fail() {
    echo -e "${RED}❌ $1${NC}"
    ALL_PASSED=false
}

check_warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

echo "=============================================="
echo "AIR-005 Deployment Verification"
echo "OpenWeatherMap Integration"
echo "=============================================="
echo ""

# ==============================================================================
# 1. Environment Variables Check
# ==============================================================================
echo "=== 1. Checking Environment Variables ==="

if [ -z "$OPENWEATHERMAP_API_KEY" ]; then
    check_fail "OPENWEATHERMAP_API_KEY not set in environment"
    echo "   Set it in deploy/pi/.env file"
else
    check_pass "OPENWEATHERMAP_API_KEY is set"
fi

if [ -n "$WEATHER_LATITUDE" ]; then
    check_pass "WEATHER_LATITUDE set to: $WEATHER_LATITUDE"
else
    check_warn "WEATHER_LATITUDE not set (will use default: 37.7749)"
fi

if [ -n "$WEATHER_LONGITUDE" ]; then
    check_pass "WEATHER_LONGITUDE set to: $WEATHER_LONGITUDE"
else
    check_warn "WEATHER_LONGITUDE not set (will use default: -122.4194)"
fi

echo ""

# ==============================================================================
# 2. Docker Services Check
# ==============================================================================
echo "=== 2. Checking Docker Services ==="

if docker compose ps | grep -q "air-quality-app.*running"; then
    check_pass "air-quality-app service is running"
else
    check_fail "air-quality-app service is not running"
fi

if docker compose ps | grep -q "etcd.*running"; then
    check_pass "etcd service is running"
else
    check_fail "etcd service is not running"
fi

if docker compose ps | grep -q "mosquitto.*running"; then
    check_pass "mosquitto service is running"
else
    check_fail "mosquitto service is not running"
fi

echo ""

# ==============================================================================
# 3. Stream Configuration Check
# ==============================================================================
echo "=== 3. Checking Stream Configurations in etcd ==="

if docker compose exec -T etcd etcdctl get /streams/outdoor-weather/config > /dev/null 2>&1; then
    check_pass "outdoor-weather stream config loaded in etcd"
else
    check_fail "outdoor-weather stream config not found in etcd"
    echo "   Load it with: ./scripts/load-stream-config.sh outdoor-weather"
fi

if docker compose exec -T etcd etcdctl get /streams/outdoor-air-quality/config > /dev/null 2>&1; then
    check_pass "outdoor-air-quality stream config loaded in etcd"
else
    check_fail "outdoor-air-quality stream config not found in etcd"
    echo "   Load it with: ./scripts/load-stream-config.sh outdoor-air-quality"
fi

echo ""

# ==============================================================================
# 4. Application Health Check
# ==============================================================================
echo "=== 4. Checking Application Health ==="

if curl -s -f http://localhost:8080/health > /dev/null 2>&1; then
    check_pass "Application health endpoint responding"

    # Pretty print health status
    HEALTH_JSON=$(curl -s http://localhost:8080/health)
    if command -v jq > /dev/null 2>&1; then
        echo ""
        echo "Health Status:"
        echo "$HEALTH_JSON" | jq .
    fi
else
    check_fail "Application health endpoint not responding"
fi

echo ""

# ==============================================================================
# 5. HTTP Polling Logs Check
# ==============================================================================
echo "=== 5. Checking HTTP Polling Logs ==="

if docker compose logs air-quality-app 2>&1 | grep -qi "http polling"; then
    check_pass "HTTP polling logs detected"

    # Show recent HTTP polling logs
    echo ""
    echo "Recent HTTP Polling Activity:"
    docker compose logs --tail=10 air-quality-app 2>&1 | grep -i "http\|weather\|polling" || true
else
    check_warn "HTTP polling logs not yet detected (may take up to 10 minutes after startup)"
fi

echo ""

# ==============================================================================
# 6. Data Directory Check
# ==============================================================================
echo "=== 6. Checking Data Directories ==="

if docker compose exec -T air-quality-app ls -d /data/outdoor-weather/ > /dev/null 2>&1; then
    check_pass "Weather data directory exists"

    # Check for parquet files
    if docker compose exec -T air-quality-app ls /data/outdoor-weather/*.parquet > /dev/null 2>&1; then
        FILE_COUNT=$(docker compose exec -T air-quality-app ls /data/outdoor-weather/*.parquet 2>/dev/null | wc -l)
        check_pass "Found $FILE_COUNT Parquet file(s) in outdoor-weather"
    else
        check_warn "No Parquet files yet in outdoor-weather (normal if < 10 min since first poll)"
    fi
else
    check_warn "Weather data directory not yet created (normal if < 10 min since startup)"
fi

if docker compose exec -T air-quality-app ls -d /data/outdoor-air-quality/ > /dev/null 2>&1; then
    check_pass "Air quality data directory exists"

    # Check for parquet files
    if docker compose exec -T air-quality-app ls /data/outdoor-air-quality/*.parquet > /dev/null 2>&1; then
        FILE_COUNT=$(docker compose exec -T air-quality-app ls /data/outdoor-air-quality/*.parquet 2>/dev/null | wc -l)
        check_pass "Found $FILE_COUNT Parquet file(s) in outdoor-air-quality"
    else
        check_warn "No Parquet files yet in outdoor-air-quality (normal if < 10 min since first poll)"
    fi
else
    check_warn "Air quality data directory not yet created (normal if < 10 min since startup)"
fi

echo ""

# ==============================================================================
# 7. Memory Usage Check
# ==============================================================================
echo "=== 7. Checking Memory Usage ==="

if command -v docker > /dev/null 2>&1; then
    MEM_USAGE=$(docker stats air-quality-app --no-stream --format "{{.MemPerc}}" | sed 's/%//')
    MEM_LIMIT=$(docker stats air-quality-app --no-stream --format "{{.MemUsage}}")

    if [ -n "$MEM_USAGE" ]; then
        if (( $(echo "$MEM_USAGE < 80" | bc -l) )); then
            check_pass "Memory usage: $MEM_LIMIT (${MEM_USAGE}% of limit)"
        else
            check_warn "Memory usage high: $MEM_LIMIT (${MEM_USAGE}% of limit)"
        fi
    fi
fi

echo ""

# ==============================================================================
# Summary
# ==============================================================================
echo "=============================================="
if [ "$ALL_PASSED" = true ]; then
    echo -e "${GREEN}✅ All critical checks passed!${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Monitor logs: docker compose logs -f air-quality-app"
    echo "  2. Check health: curl http://localhost:8080/health | jq"
    echo "  3. Wait 10 minutes for first weather data to appear"
    exit 0
else
    echo -e "${RED}❌ Some checks failed. Review errors above.${NC}"
    echo ""
    echo "Common fixes:"
    echo "  1. Set OPENWEATHERMAP_API_KEY in deploy/pi/.env"
    echo "  2. Load stream configs: ./scripts/load-stream-config.sh outdoor-weather"
    echo "  3. Restart services: docker compose restart"
    exit 1
fi

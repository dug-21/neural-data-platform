#!/bin/bash
# Grafana Integration Verification Script
# Feature: DP-001
# Author: ndp-grafana-dev

set -e

echo "========================================="
echo "Grafana Integration Verification"
echo "========================================="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

success() {
    echo -e "${GREEN}✓${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
}

warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# 1. Check Docker services
echo "1. Checking Docker services..."
if docker compose -f deploy/pi/docker-compose.yml ps | grep -q "duckdb.*Up"; then
    success "DuckDB container is running"
else
    error "DuckDB container is not running"
    exit 1
fi

if docker compose -f deploy/pi/docker-compose.yml ps | grep -q "grafana.*Up"; then
    success "Grafana container is running"
else
    error "Grafana container is not running"
    exit 1
fi
echo ""

# 2. Check DuckDB database
echo "2. Checking DuckDB database..."
if docker compose -f deploy/pi/docker-compose.yml exec -T duckdb test -f /duckdb/neural_platform.db; then
    success "DuckDB database exists"
else
    error "DuckDB database not found"
    exit 1
fi
echo ""

# 3. Check DuckDB views
echo "3. Checking DuckDB views..."
views=$(docker compose -f deploy/pi/docker-compose.yml exec -T duckdb duckdb /duckdb/neural_platform.db \
    "SELECT COUNT(*) FROM information_schema.tables WHERE table_type = 'VIEW'" 2>/dev/null | tail -1)

if [ "$views" -ge 4 ]; then
    success "DuckDB views created ($views views found)"
else
    warning "Expected 4+ views, found $views"
fi

# Check specific views
for view in silver_indoor_air silver_outdoor_weather silver_outdoor_air readings_hourly; do
    if docker compose -f deploy/pi/docker-compose.yml exec -T duckdb duckdb /duckdb/neural_platform.db \
        "SELECT 1 FROM information_schema.tables WHERE table_name = '$view'" 2>/dev/null | grep -q 1; then
        success "View exists: $view"
    else
        error "View missing: $view"
    fi
done
echo ""

# 4. Check SQLite export
echo "4. Checking SQLite export..."
if docker compose -f deploy/pi/docker-compose.yml exec -T duckdb test -f /duckdb/grafana.db; then
    size=$(docker compose -f deploy/pi/docker-compose.yml exec -T duckdb stat -c%s /duckdb/grafana.db 2>/dev/null || echo 0)
    if [ "$size" -gt 1024 ]; then
        success "SQLite export exists ($(numfmt --to=iec $size 2>/dev/null || echo "$size bytes"))"
    else
        warning "SQLite export exists but is very small ($size bytes)"
    fi
else
    error "SQLite export not found"
    echo "  Waiting for first export (max 5 minutes)..."
fi
echo ""

# 5. Check readings_hourly data
echo "5. Checking readings_hourly data..."
rows=$(docker compose -f deploy/pi/docker-compose.yml exec -T duckdb duckdb /duckdb/neural_platform.db \
    "SELECT COUNT(*) FROM readings_hourly" 2>/dev/null | tail -1 || echo 0)

if [ "$rows" -gt 0 ]; then
    success "readings_hourly has $rows rows"

    # Check by stream
    for stream in air-quality outdoor-conditions outdoor-air-quality; do
        stream_rows=$(docker compose -f deploy/pi/docker-compose.yml exec -T duckdb duckdb /duckdb/neural_platform.db \
            "SELECT COUNT(*) FROM readings_hourly WHERE stream_id = '$stream'" 2>/dev/null | tail -1 || echo 0)
        if [ "$stream_rows" -gt 0 ]; then
            success "  $stream: $stream_rows rows"
        else
            warning "  $stream: no data"
        fi
    done
else
    error "readings_hourly is empty"
    echo "  This may be normal if no data has been ingested yet"
fi
echo ""

# 6. Check Grafana datasource
echo "6. Checking Grafana datasource..."
if docker compose -f deploy/pi/docker-compose.yml exec -T grafana test -f /data/duckdb/grafana.db; then
    success "Grafana can access SQLite database"
else
    error "Grafana cannot access SQLite database"
fi

# Check datasource provisioning
if docker compose -f deploy/pi/docker-compose.yml exec -T grafana test -f /etc/grafana/provisioning/datasources/duckdb.yaml; then
    success "Datasource provisioning file exists"
else
    error "Datasource provisioning file not found"
fi
echo ""

# 7. Check Grafana dashboards
echo "7. Checking Grafana dashboards..."
dashboard_count=$(docker compose -f deploy/pi/docker-compose.yml exec -T grafana find /var/lib/grafana/dashboards -name "*.json" 2>/dev/null | wc -l)

if [ "$dashboard_count" -gt 0 ]; then
    success "Found $dashboard_count dashboard(s)"
    docker compose -f deploy/pi/docker-compose.yml exec -T grafana find /var/lib/grafana/dashboards -name "*.json" 2>/dev/null | while read dashboard; do
        success "  $(basename $dashboard)"
    done
else
    error "No dashboards found"
fi
echo ""

# 8. Check Grafana API
echo "8. Checking Grafana API..."
if curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/health | grep -q 200; then
    success "Grafana API is responding"
else
    error "Grafana API is not responding"
fi
echo ""

# 9. Summary
echo "========================================="
echo "Verification Summary"
echo "========================================="
echo ""
echo "Next steps:"
echo "1. Access Grafana at http://localhost:3000"
echo "2. Login with admin / (GRAFANA_ADMIN_PASSWORD)"
echo "3. Navigate to Dashboards → Neural Data Platform"
echo "4. Verify data is displaying correctly"
echo ""
echo "If no data is showing:"
echo "- Wait 5 minutes for first SQLite export"
echo "- Check logs: docker compose -f deploy/pi/docker-compose.yml logs duckdb"
echo "- Verify data ingestion is working"
echo ""

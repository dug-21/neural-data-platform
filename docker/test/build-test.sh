#!/bin/bash
set -e

echo "Building Neural Trader Test Environment"
echo "======================================"

# Get the directory of this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Set default tag
TAG="${1:-test}"
echo "Building with tag: $TAG"

# Build all test images
echo ""
echo "Building TimescaleDB test image..."
docker build -f "$SCRIPT_DIR/images/timescaledb-test.dockerfile" -t "neural-trader/timescaledb:$TAG" "$SCRIPT_DIR"

echo ""
echo "Building Neural Trader test image..."
docker build -f "$SCRIPT_DIR/images/neural-trader-test.dockerfile" -t "neural-trader/app:$TAG" "$PROJECT_ROOT"

echo ""
echo "Building Data Ingestion test image..."
docker build -f "$SCRIPT_DIR/images/data-ingestion-test.dockerfile" -t "neural-trader/data-ingestion:$TAG" "$PROJECT_ROOT"

echo ""
echo "Building Test Data Generator image..."
docker build -f "$SCRIPT_DIR/images/test-data-generator.dockerfile" -t "neural-trader/test-data-generator:$TAG" "$PROJECT_ROOT"

echo ""
echo "Building Mock API Server image..."
docker build -f "$SCRIPT_DIR/images/mock-api-server.dockerfile" -t "neural-trader/mock-api-server:$TAG" "$PROJECT_ROOT"

echo ""
echo "Building Prometheus test image..."
docker build -f "$SCRIPT_DIR/images/prometheus-test.dockerfile" -t "neural-trader/prometheus:$TAG" "$SCRIPT_DIR"

echo ""
echo "Building Grafana test image..."
docker build -f "$SCRIPT_DIR/images/grafana-test.dockerfile" -t "neural-trader/grafana:$TAG" "$SCRIPT_DIR"

echo ""
echo "Test images built successfully!"
echo ""
echo "Available images:"
docker images | grep "neural-trader.*$TAG"

echo ""
echo "To start the test environment:"
echo "  cd $SCRIPT_DIR"
echo "  docker-compose -f docker-compose.test.yml up -d"
echo ""
echo "To run tests:"
echo "  docker-compose -f docker-compose.test.yml exec neural-trader-test /usr/local/bin/test-runner.sh"
echo ""
echo "To generate test data:"
echo "  docker-compose -f docker-compose.test.yml up test-data-generator"
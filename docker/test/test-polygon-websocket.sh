#!/bin/bash
# Test Polygon WebSocket integration without rebuilding images

echo "🧪 Testing Polygon WebSocket Integration"
echo "========================================"

# Check if API key is provided
if [ -z "$POLYGON_API_KEY" ]; then
    echo "❌ Error: POLYGON_API_KEY not set"
    echo "Please run: export POLYGON_API_KEY='your-api-key'"
    exit 1
fi

# Run test with volume mount and real API key
docker-compose -f docker-compose.test.yml run --rm \
  -v $(pwd)/../../data_ingestion:/app/data_ingestion:ro \
  -e POLYGON_API_KEY=$POLYGON_API_KEY \
  -e POLYGON_WEBSOCKET_ENABLED=true \
  -e PRIMARY_PROVIDER=polygon \
  -e DEFAULT_PROVIDER=polygon \
  -e MOCK_DATA_ENABLED=false \
  -e LOG_LEVEL=INFO \
  data-ingestion-test python -m data_ingestion.providers.test_polygon_websocket

echo "✅ Test completed"
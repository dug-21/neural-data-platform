#!/bin/bash
# Test script for health check endpoints

echo "Testing Health Check Endpoints"
echo "=============================="

# Function to test endpoint
test_endpoint() {
    local endpoint=$1
    local description=$2
    
    echo -e "\n$description"
    echo "Endpoint: $endpoint"
    
    # Test with curl
    response=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080$endpoint)
    echo "HTTP Status: $response"
    
    # Get full response
    echo "Response:"
    curl -s http://localhost:8080$endpoint | python -m json.tool 2>/dev/null || curl -s http://localhost:8080$endpoint
    echo ""
}

# Test all endpoints
test_endpoint "/health" "1. Simple Health Check"
test_endpoint "/health/detailed" "2. Detailed Health Check"
test_endpoint "/health/live" "3. Liveness Probe"
test_endpoint "/health/ready" "4. Readiness Probe"

echo -e "\nPrometheus Metrics:"
echo "==================="
echo "Health metrics available at: http://localhost:8001/metrics"
echo ""
echo "Key metrics to monitor:"
echo "- data_ingestion_health_status"
echo "- data_ingestion_health_component_status"
echo "- data_ingestion_data_flow_age_seconds"
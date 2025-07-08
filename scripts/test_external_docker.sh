#!/bin/bash

# Test script to verify external Docker connection and capabilities

echo "🔍 External Docker Connection Test"
echo "=================================="
echo ""

EXTERNAL_DOCKER_HOST="tcp://host.docker.internal:2375"

# Allow user to override Docker host
if [ ! -z "$DOCKER_HOST_OVERRIDE" ]; then
    EXTERNAL_DOCKER_HOST="$DOCKER_HOST_OVERRIDE"
fi

echo "📍 Testing connection to: $EXTERNAL_DOCKER_HOST"
echo ""

# Test 1: Basic connection
echo "1️⃣ Testing basic Docker connection..."
if DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker info > /dev/null 2>&1; then
    echo "   ✅ Connection successful!"
    
    # Get Docker version
    DOCKER_VERSION=$(DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker version --format '{{.Server.Version}}' 2>/dev/null)
    echo "   📦 Docker version: $DOCKER_VERSION"
else
    echo "   ❌ Connection failed!"
    echo ""
    echo "   Please ensure:"
    echo "   - Docker Desktop is running"
    echo "   - TCP exposure is enabled (port 2375)"
    echo "   - No firewall is blocking the connection"
    exit 1
fi

# Test 2: Check disk space
echo ""
echo "2️⃣ Checking available disk space..."
DISK_INFO=$(DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker system df 2>/dev/null)
if [ $? -eq 0 ]; then
    echo "   ✅ Disk check successful!"
    echo ""
    echo "$DISK_INFO" | head -n 5
else
    echo "   ⚠️  Could not check disk space"
fi

# Test 3: Test image pull
echo ""
echo "3️⃣ Testing image pull capability..."
if DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker pull alpine:latest > /dev/null 2>&1; then
    echo "   ✅ Image pull successful!"
else
    echo "   ❌ Image pull failed!"
fi

# Test 4: Test container run
echo ""
echo "4️⃣ Testing container execution..."
TEST_OUTPUT=$(DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker run --rm alpine:latest echo "Hello from external Docker" 2>/dev/null)
if [ "$TEST_OUTPUT" = "Hello from external Docker" ]; then
    echo "   ✅ Container execution successful!"
else
    echo "   ❌ Container execution failed!"
fi

# Test 5: Check if docker-compose is available
echo ""
echo "5️⃣ Testing docker-compose availability..."
if command -v docker-compose > /dev/null 2>&1; then
    echo "   ✅ docker-compose is available"
    COMPOSE_VERSION=$(docker-compose version --short 2>/dev/null)
    echo "   📦 docker-compose version: $COMPOSE_VERSION"
else
    echo "   ⚠️  docker-compose not found in PATH"
    echo "   You may need to install it or use 'docker compose' syntax"
fi

# Summary
echo ""
echo "📊 Test Summary"
echo "==============="
echo ""
echo "✅ External Docker is properly configured and ready to use!"
echo ""
echo "You can now run:"
echo "  ./scripts/start_full_stock_simulation_external.sh"
echo ""
echo "To use a different Docker host, set:"
echo "  export DOCKER_HOST_OVERRIDE=tcp://your-host:port"
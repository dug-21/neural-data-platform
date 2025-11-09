# Module Testing Scripts Specification

## Overview

This document specifies the helper scripts needed to support module-specific testing in the CICD pipeline.

## Core Scripts

### module-setup.sh
```bash
#!/bin/bash
# Setup environment for specific module testing

set -e

MODULE=$1
CONFIG_ENV=${CONFIG_ENV:-dev}

echo "Setting up module: $MODULE"

# Detect module type
if [ "$MODULE" = "data-ingestion" ] || [ "$MODULE" = "data_ingestion" ]; then
    MODULE_TYPE="python"
    MODULE_DIR="data_ingestion"
else
    MODULE_TYPE="rust"
    MODULE_DIR="$MODULE"
fi

# Define required services
case $MODULE in
    config-store)
        REQUIRED_SERVICES="redis timescaledb"
        ;;
    data-ingestion|data_ingestion)
        REQUIRED_SERVICES="redis timescaledb config-store"
        ;;
    data-staging)
        REQUIRED_SERVICES="redis config-store"
        ;;
    neural-ml-ops)
        REQUIRED_SERVICES="redis timescaledb config-store"
        ;;
    neural-trading)
        REQUIRED_SERVICES="redis config-store"
        ;;
    *)
        echo "Unknown module: $MODULE"
        exit 1
        ;;
esac

# Start only required services
echo "Starting required services: $REQUIRED_SERVICES"
docker-compose -f docker-compose.v2.yml up -d $REQUIRED_SERVICES

# Wait for services to be healthy
for service in $REQUIRED_SERVICES; do
    echo "Waiting for $service to be healthy..."
    ./scripts/wait-for-service.sh $service
done

# Special case: seed config-store if needed
if [[ " $REQUIRED_SERVICES " =~ " config-store " ]]; then
    echo "Seeding config-store..."
    ./scripts/seed-config-store.sh $CONFIG_ENV
fi

echo "Module setup complete for $MODULE"
```

### module-build.sh
```bash
#!/bin/bash
# Build specific module

set -e

MODULE=$1
VERBOSE=${VERBOSE:-false}

echo "Building module: $MODULE"

# Detect module type and build
if [ "$MODULE" = "data-ingestion" ] || [ "$MODULE" = "data_ingestion" ]; then
    echo "Building Python module..."
    cd data_ingestion
    
    # Create virtual environment if needed
    if [ ! -d "venv" ]; then
        python3 -m venv venv
    fi
    
    source venv/bin/activate
    pip install -r requirements.txt
    
    if [ "$VERBOSE" = "true" ]; then
        pip list
    fi
else
    echo "Building Rust module..."
    BUILD_FLAGS="--release -p $MODULE"
    
    if [ "$VERBOSE" = "true" ]; then
        BUILD_FLAGS="$BUILD_FLAGS -v"
    fi
    
    cargo build $BUILD_FLAGS
fi

echo "Build complete for $MODULE"
```

### module-test.sh
```bash
#!/bin/bash
# Run unit tests for specific module

set -e

MODULE=$1
VERBOSE=${VERBOSE:-false}

echo "Testing module: $MODULE"

# Detect module type and test
if [ "$MODULE" = "data-ingestion" ] || [ "$MODULE" = "data_ingestion" ]; then
    echo "Running Python tests..."
    cd data_ingestion
    source venv/bin/activate
    
    TEST_FLAGS=""
    if [ "$VERBOSE" = "true" ]; then
        TEST_FLAGS="-v"
    fi
    
    pytest tests/unit/ $TEST_FLAGS --cov=$MODULE --cov-report=html
else
    echo "Running Rust tests..."
    TEST_FLAGS="-p $MODULE"
    
    if [ "$VERBOSE" = "true" ]; then
        TEST_FLAGS="$TEST_FLAGS -- --nocapture"
    fi
    
    cargo test $TEST_FLAGS
    
    # Generate coverage if requested
    if [ "$COVERAGE" = "true" ]; then
        cargo tarpaulin -p $MODULE --out Html --output-dir coverage/$MODULE
    fi
fi

echo "Tests complete for $MODULE"
```

### module-integration.sh
```bash
#!/bin/bash
# Run integration tests for specific module

set -e

MODULE=$1
KEEP_ALIVE=${KEEP_ALIVE:-false}

echo "Running integration tests for: $MODULE"

# Build container for module
docker-compose -f docker-compose.v2.yml build $MODULE

# Start module container
docker-compose -f docker-compose.v2.yml up -d $MODULE

# Wait for module to be healthy
./scripts/wait-for-service.sh $MODULE

# Run module-specific integration tests
case $MODULE in
    config-store)
        # Test config loading and gRPC API
        docker-compose -f docker-compose.v2.yml run --rm test-runner \
            python tests/integration/test_config_store.py
        ;;
    data-staging)
        # Test data flow and proto transformation
        docker-compose -f docker-compose.v2.yml run --rm test-runner \
            python tests/integration/test_data_staging.py
        ;;
    neural-trading)
        # Test trading logic and EventBus integration
        docker-compose -f docker-compose.v2.yml run --rm test-runner \
            python tests/integration/test_neural_trading.py
        ;;
    neural-ml-ops)
        # Test ML pipeline and feature engineering
        docker-compose -f docker-compose.v2.yml run --rm test-runner \
            python tests/integration/test_neural_ml_ops.py
        ;;
    data-ingestion|data_ingestion)
        # Test data ingestion and storage
        docker-compose -f docker-compose.v2.yml run --rm test-runner \
            python tests/integration/test_data_ingestion.py
        ;;
esac

# Cleanup unless debugging
if [ "$KEEP_ALIVE" != "true" ]; then
    docker-compose -f docker-compose.v2.yml stop $MODULE
fi

echo "Integration tests complete for $MODULE"
```

### module-report.sh
```bash
#!/bin/bash
# Generate test report for specific module

set -e

MODULE=$1
OUTPUT_DIR=${OUTPUT_DIR:-"test-results"}

echo "Generating report for: $MODULE"

# Create output directory
mkdir -p $OUTPUT_DIR/$MODULE

# Collect test results
if [ "$MODULE" = "data-ingestion" ] || [ "$MODULE" = "data_ingestion" ]; then
    # Python module report
    cp -r data_ingestion/htmlcov $OUTPUT_DIR/$MODULE/coverage
    cp data_ingestion/.coverage $OUTPUT_DIR/$MODULE/
    
    # Generate JSON report
    cd data_ingestion
    source venv/bin/activate
    coverage json -o ../$OUTPUT_DIR/$MODULE/coverage.json
else
    # Rust module report
    if [ -d "coverage/$MODULE" ]; then
        cp -r coverage/$MODULE $OUTPUT_DIR/$MODULE/coverage
    fi
    
    # Collect test output
    cargo test -p $MODULE --no-run 2>&1 | tee $OUTPUT_DIR/$MODULE/test_output.log
fi

# Generate summary
cat > $OUTPUT_DIR/$MODULE/summary.json <<EOF
{
  "module": "$MODULE",
  "timestamp": "$(date -Iseconds)",
  "test_results": {
    "passed": $(grep -c "test result: ok" $OUTPUT_DIR/$MODULE/test_output.log || echo 0),
    "failed": $(grep -c "test result: FAILED" $OUTPUT_DIR/$MODULE/test_output.log || echo 0)
  },
  "duration": "$SECONDS seconds"
}
EOF

echo "Report generated at: $OUTPUT_DIR/$MODULE"
```

### wait-for-service.sh
```bash
#!/bin/bash
# Wait for a service to be healthy

set -e

SERVICE=$1
MAX_ATTEMPTS=${MAX_ATTEMPTS:-30}
SLEEP_TIME=${SLEEP_TIME:-2}

echo "Waiting for $SERVICE to be healthy..."

attempt=1
while [ $attempt -le $MAX_ATTEMPTS ]; do
    case $SERVICE in
        redis)
            if docker-compose -f docker-compose.v2.yml exec -T redis redis-cli ping > /dev/null 2>&1; then
                echo "$SERVICE is healthy"
                exit 0
            fi
            ;;
        timescaledb)
            if docker-compose -f docker-compose.v2.yml exec -T timescaledb pg_isready -U postgres > /dev/null 2>&1; then
                echo "$SERVICE is healthy"
                exit 0
            fi
            ;;
        config-store)
            if curl -f http://localhost:8090/health > /dev/null 2>&1; then
                echo "$SERVICE is healthy"
                exit 0
            fi
            ;;
        *)
            # Generic health check using docker
            if docker-compose -f docker-compose.v2.yml ps $SERVICE | grep -q "healthy"; then
                echo "$SERVICE is healthy"
                exit 0
            fi
            ;;
    esac
    
    echo "Attempt $attempt/$MAX_ATTEMPTS: $SERVICE not ready, waiting..."
    sleep $SLEEP_TIME
    attempt=$((attempt + 1))
done

echo "ERROR: $SERVICE failed to become healthy after $MAX_ATTEMPTS attempts"
exit 1
```

## Module Dependencies Matrix

| Module | Build Dependencies | Runtime Dependencies | Test Dependencies |
|--------|-------------------|---------------------|-------------------|
| config-store | - | Redis, TimescaleDB | Redis, TimescaleDB |
| data-ingestion | requests, pydantic | Redis, TimescaleDB, config-store | All + mock data |
| data-staging | neural-core | Redis, config-store | Redis, config-store |
| neural-ml-ops | neural-core | Redis, TimescaleDB, config-store, EventBus | All services |
| neural-trading | neural-core | Redis, config-store, EventBus | config-store, EventBus |

## Usage Examples

### Single Module Testing
```bash
# Test config-store module
./scripts/module-setup.sh config-store
./scripts/module-build.sh config-store
./scripts/module-test.sh config-store
./scripts/module-integration.sh config-store
./scripts/module-report.sh config-store
```

### Parallel Module Testing
```bash
# Test multiple modules in parallel
parallel -j 3 ::: \
    "./scripts/module-test.sh config-store" \
    "./scripts/module-test.sh data-staging" \
    "./scripts/module-test.sh neural-trading"
```

### CI/CD Integration
```bash
# In GitHub Actions or local CI
for module in config-store data-staging neural-trading; do
    echo "Testing $module..."
    ./scripts/module-setup.sh $module
    ./scripts/module-build.sh $module
    ./scripts/module-test.sh $module
    ./scripts/module-integration.sh $module
    ./scripts/module-report.sh $module
    ./scripts/module-teardown.sh $module
done
```

## Performance Optimization

### Caching Strategy
- Cache Docker layers per module
- Cache Rust dependencies per module
- Cache Python virtual environments
- Reuse running services between tests

### Parallel Execution
- Run independent modules in parallel
- Share infrastructure services
- Use separate test databases per module

## Error Handling

### Failure Modes
- Service startup failure: Retry with backoff
- Test failure: Continue other modules
- Build failure: Stop pipeline for module
- Integration failure: Capture logs and continue

### Debugging Support
- KEEP_ALIVE flag to preserve environment
- VERBOSE flag for detailed output
- Module-specific log collection
- Container state snapshots

## Next Steps

1. Implement core scripts
2. Test with each module
3. Add parallel execution support
4. Create CI/CD integration
5. Document module-specific quirks
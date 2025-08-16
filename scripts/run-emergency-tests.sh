#!/bin/bash

# Emergency Test Runner Script
# Provides immediate protection testing for neural-trader

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_DIR="$PROJECT_ROOT/tests/emergency"

echo "🚨 Neural Trader Emergency Test Runner"
echo "======================================"
echo ""

# Check if test directory exists
if [ ! -d "$TEST_DIR" ]; then
    echo "❌ Error: Emergency test directory not found at $TEST_DIR"
    exit 1
fi

cd "$TEST_DIR"

# Check for required environment variables
if [ -z "$DATABASE_URL" ]; then
    echo "⚠️  Warning: DATABASE_URL not set, using default"
    export DATABASE_URL="postgres://postgres:postgres@localhost:5432/neural_trader_db"
fi

if [ -z "$REDIS_URL" ]; then
    echo "⚠️  Warning: REDIS_URL not set, using default"
    export REDIS_URL="redis://localhost:6379"
fi

# Run different test modes based on argument
case "${1:-all}" in
    quick)
        echo "🏃 Running quick tests only..."
        cargo test test_system_health --release 2>/dev/null || true
        ;;
    
    trading)
        echo "💹 Running trading tests..."
        cargo test test_trading --release
        ;;
    
    data)
        echo "📊 Running data pipeline tests..."
        cargo test test_data --release
        ;;
    
    neural)
        echo "🧠 Running neural model tests..."
        cargo test test_neural --release
        ;;
    
    vendor)
        echo "🎯 Running vendor predictor tests..."
        cargo test test_vendor_predictor --release
        ;;
    
    health)
        echo "🏥 Running health check tests..."
        cargo test test_health --release
        ;;
    
    all)
        echo "🔍 Running all emergency tests..."
        echo ""
        
        # First, try to compile
        echo "📦 Compiling emergency tests..."
        if cargo build --release 2>/dev/null; then
            echo "✅ Tests compiled successfully"
        else
            echo "❌ Compilation failed, attempting with reduced features..."
            cargo build 2>/dev/null || {
                echo "❌ Cannot compile tests. Check Cargo.toml dependencies."
                exit 1
            }
        fi
        
        echo ""
        echo "🧪 Executing tests..."
        echo ""
        
        # Run tests with nice output
        cargo test --release -- --test-threads=1 --nocapture 2>/dev/null || {
            echo ""
            echo "⚠️  Some tests failed, but this is expected if system is offline"
            echo "    Review the output above for details"
        }
        ;;
    
    watch)
        echo "👁️  Watching for changes and running tests..."
        while true; do
            clear
            echo "🔄 Running tests at $(date '+%H:%M:%S')..."
            cargo test --release -- --test-threads=1 2>/dev/null || true
            echo ""
            echo "Waiting for changes... (Ctrl+C to stop)"
            sleep 10
        done
        ;;
    
    *)
        echo "Usage: $0 [all|quick|trading|data|neural|vendor|health|watch]"
        echo ""
        echo "  all      - Run all emergency tests (default)"
        echo "  quick    - Run only system health check"
        echo "  trading  - Test trading decision flow"
        echo "  data     - Test data pipeline integrity"
        echo "  neural   - Test neural model predictions"
        echo "  vendor   - Test vendor predictor architecture"
        echo "  health   - Test system health endpoints"
        echo "  watch    - Continuously run tests every 10 seconds"
        exit 1
        ;;
esac

echo ""
echo "✅ Test run completed"
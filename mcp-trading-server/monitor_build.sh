#!/bin/bash

# Build monitoring script for mcp-trading-server

echo "=== MCP Trading Server Build Monitor ==="
echo "Started at: $(date)"
echo

while true; do
    echo -n "Checking build status... "
    
    # Run cargo check and capture output
    if cargo check 2>&1 | tee /tmp/current_build.log | grep -q "error: could not compile"; then
        ERROR_COUNT=$(grep -c "error\[E[0-9]\+\]:" /tmp/current_build.log)
        WARNING_COUNT=$(grep -c "warning:" /tmp/current_build.log)
        echo "FAILED - Errors: $ERROR_COUNT, Warnings: $WARNING_COUNT"
        
        # Log progress
        echo "[$(date)] Errors: $ERROR_COUNT, Warnings: $WARNING_COUNT" >> /tmp/build_validation.log
        
        # Show current error summary
        echo "Current errors:"
        grep "error\[E[0-9]\+\]:" /tmp/current_build.log | head -5
        echo
    else
        echo "SUCCESS - Build completed!"
        echo "[$(date)] BUILD SUCCESS!" >> /tmp/build_validation.log
        
        # Run additional checks
        echo "Running cargo test --no-run..."
        if cargo test --no-run 2>&1 | tee /tmp/test_build.log; then
            echo "Test build: SUCCESS"
            echo "[$(date)] Test build: SUCCESS" >> /tmp/build_validation.log
        else
            echo "Test build: FAILED"
            echo "[$(date)] Test build: FAILED" >> /tmp/build_validation.log
        fi
        
        break
    fi
    
    # Wait before next check
    sleep 10
done

echo
echo "=== Final Validation Report ==="
cat /tmp/build_validation.log
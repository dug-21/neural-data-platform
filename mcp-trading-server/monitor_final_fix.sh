#!/bin/bash

echo "Monitoring for final PositionSize fix..."

while true; do
    if cargo check > /tmp/final_check.txt 2>&1; then
        echo "[$(date)] BUILD SUCCESS! All errors fixed!"
        echo "[$(date)] BUILD SUCCESS! All errors fixed!" >> /tmp/build_validation.log
        
        # Run additional validation
        echo "Running cargo test --no-run..."
        if cargo test --no-run; then
            echo "[$(date)] Test compilation: SUCCESS" >> /tmp/build_validation.log
        else
            echo "[$(date)] Test compilation: FAILED" >> /tmp/build_validation.log
        fi
        
        # Check main app
        echo "Checking main application..."
        cd /workspaces/neural-trader
        if cargo check; then
            echo "[$(date)] Main app compilation: SUCCESS" >> /tmp/build_validation.log
        else  
            echo "[$(date)] Main app compilation: FAILED" >> /tmp/build_validation.log
        fi
        
        echo "=== FINAL VALIDATION COMPLETE ==="
        cat /tmp/build_validation.log
        break
    else
        ERRORS=$(grep -c "error\[E[0-9]\+\]:" /tmp/final_check.txt 2>/dev/null || echo "0")
        if [ "$ERRORS" != "1" ]; then
            echo "[$(date)] Error count changed to: $ERRORS"
            echo "[$(date)] Error count changed to: $ERRORS" >> /tmp/build_validation.log
        fi
    fi
    sleep 5
done
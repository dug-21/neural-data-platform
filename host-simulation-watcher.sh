#!/bin/bash
# Run this script on your HOST machine (not in the container)
# It watches for simulation requests from the dev container

echo "👀 Watching for simulation launch requests..."
echo "   (Keep this terminal open)"
echo ""

WATCH_FILE="/Users/dmf/repos/neural-trader/.simulation-request.json"
PROCESSED_FILE="/Users/dmf/repos/neural-trader/.simulation-processed"

# Function to launch simulation
launch_simulation() {
    echo "🚀 Launch request detected! Starting simulation..."
    
    # Mark as processed
    touch "$PROCESSED_FILE"
    
    # Run the simulation script
    /Users/dmf/repos/neural-trader/run-simulation-host.sh
    
    # Clean up request file
    rm -f "$WATCH_FILE"
    rm -f "$PROCESSED_FILE"
}

# Watch for changes
while true; do
    if [ -f "$WATCH_FILE" ] && [ ! -f "$PROCESSED_FILE" ]; then
        launch_simulation
    fi
    sleep 2
done
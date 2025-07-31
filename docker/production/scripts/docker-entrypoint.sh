#!/bin/bash
# Docker entrypoint for neural-trader with model initialization

set -e

# Run model initialization
echo "Running model storage initialization..."
/app/scripts/init-models.sh

# Check if initialization was successful
if [ ! -f "${MODEL_STORAGE_PATH:-/app/models}/.initialized" ]; then
    echo "ERROR: Model storage initialization failed!"
    exit 1
fi

echo "Starting neural-trader application..."

# Execute the main application
exec "$@"
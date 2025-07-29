#!/bin/bash
# Initialize model directory structure for Docker volumes

set -e

MODEL_BASE_PATH="${MODEL_STORAGE_PATH:-/app/models}"

echo "Initializing model storage at ${MODEL_BASE_PATH}..."

# Create directory structure
directories=(
    "${MODEL_BASE_PATH}/checkpoints/NHITS"
    "${MODEL_BASE_PATH}/checkpoints/TCN"
    "${MODEL_BASE_PATH}/checkpoints/DeepAR"
    "${MODEL_BASE_PATH}/checkpoints/MLP"
    "${MODEL_BASE_PATH}/production/NHITS"
    "${MODEL_BASE_PATH}/production/MLP"
    "${MODEL_BASE_PATH}/archive"
    "${MODEL_BASE_PATH}/backups"
)

for dir in "${directories[@]}"; do
    if [ ! -d "$dir" ]; then
        echo "Creating directory: $dir"
        mkdir -p "$dir"
    fi
done

# Set proper permissions (if running as root)
if [ "$(id -u)" = "0" ]; then
    echo "Setting permissions for neuraltrader user..."
    chown -R neuraltrader:neuraltrader "${MODEL_BASE_PATH}"
fi

# Create a marker file to indicate initialization
touch "${MODEL_BASE_PATH}/.initialized"
echo "Model storage initialization complete at $(date)"

# Check for existing models
echo "Checking for existing models..."
model_count=$(find "${MODEL_BASE_PATH}" -name "metadata.json" -type f 2>/dev/null | wc -l)
echo "Found ${model_count} existing models"

# Log model directory status
echo "Model directory status:"
ls -la "${MODEL_BASE_PATH}/"

exit 0
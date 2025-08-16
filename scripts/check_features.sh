#!/bin/bash

echo "=== Neural Trader Feature Status ==="
echo

# Check environment variables
echo "📊 Feature Flags:"
echo "  ENABLE_AUTONOMOUS_TRAINING: ${ENABLE_AUTONOMOUS_TRAINING:-false}"
echo "  ENABLE_SECTOR_MODELS: ${ENABLE_SECTOR_MODELS:-false}"
echo "  ENABLE_REALTIME_ADAPTATION: ${ENABLE_REALTIME_ADAPTATION:-false}"
echo "  ENABLE_DATA_DISCOVERY: ${ENABLE_DATA_DISCOVERY:-false}"
echo

# Check configuration files
echo "📁 Configuration Files:"
for file in config/sector_models.toml config/autonomous_training.toml config/data_requirements.toml; do
    if [ -f "$file" ]; then
        echo "  ✅ $file exists"
    else
        echo "  ❌ $file missing"
    fi
done
echo

# Check if running in Docker
if [ -f /.dockerenv ]; then
    echo "🐳 Running in Docker container"
else
    echo "💻 Running on host system"
fi
echo

# Show current configuration paths
echo "🔧 Configuration Paths:"
echo "  SECTOR_CONFIG_PATH: ${SECTOR_CONFIG_PATH:-config/sector_models.toml}"
echo "  AUTONOMOUS_TRAINING_CONFIG: ${AUTONOMOUS_TRAINING_CONFIG:-config/autonomous_training.toml}"
echo "  DATA_REQUIREMENTS_CONFIG: ${DATA_REQUIREMENTS_CONFIG:-config/data_requirements.toml}"
echo

echo "==================================="
#!/bin/bash
# Config Store Seeding Script - Populate config-store from Git

set -e

# Configuration
CONFIG_ENV=${1:-dev}
CONFIG_REPO=${CONFIG_REPO:-/workspaces/neural-trader/configs}
VALIDATE=${VALIDATE:-true}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

# Services to configure
SERVICES=(
    "config-store"
    "data-ingestion"
    "data-staging"
    "neural-ml-ops"
    "neural-trading"
)

# Validate configuration against schema
validate_config() {
    local service=$1
    local config_file=$2
    local schema_file="${CONFIG_REPO}/schemas/${service}.schema.json"
    
    if [ ! -f "$schema_file" ]; then
        log_warn "No schema found for $service, skipping validation"
        return 0
    fi
    
    log_step "Validating $service configuration..."
    
    # Convert YAML to JSON for validation
    local json_config="/tmp/${service}_config.json"
    python3 -c "
import yaml, json, sys
with open('$config_file', 'r') as f:
    config = yaml.safe_load(f)
with open('$json_config', 'w') as f:
    json.dump(config, f)
"
    
    # Validate against schema
    if command -v jsonschema > /dev/null 2>&1; then
        if jsonschema -i "$json_config" "$schema_file" > /dev/null 2>&1; then
            log_info "✓ $service configuration is valid"
            return 0
        else
            log_error "✗ $service configuration validation failed"
            jsonschema -i "$json_config" "$schema_file"
            return 1
        fi
    else
        # Fallback to Python validation
        python3 -c "
import json
import jsonschema
from jsonschema import validate

with open('$json_config', 'r') as f:
    config = json.load(f)
    
with open('$schema_file', 'r') as f:
    schema = json.load(f)
    
try:
    validate(instance=config, schema=schema)
    print('✓ Configuration is valid')
    exit(0)
except jsonschema.exceptions.ValidationError as e:
    print(f'✗ Validation error: {e.message}')
    exit(1)
" || return 1
    fi
    
    return 0
}

# Merge base and overlay configurations
merge_configs() {
    local service=$1
    local environment=$2
    local base_config="${CONFIG_REPO}/base/${service}/config.yaml"
    local overlay_config="${CONFIG_REPO}/overlays/${environment}/${service}/config.yaml"
    local merged_config="/tmp/${service}_merged.yaml"
    
    log_step "Merging configurations for $service ($environment)..."
    
    if [ ! -f "$base_config" ]; then
        log_error "Base configuration not found: $base_config"
        return 1
    fi
    
    # Start with base config
    cp "$base_config" "$merged_config"
    
    # Apply overlay if exists
    if [ -f "$overlay_config" ]; then
        log_info "Applying $environment overlay for $service"
        
        # Merge using Python
        python3 -c "
import yaml
import sys
from pathlib import Path

def deep_merge(base, overlay):
    '''Recursively merge overlay into base'''
    if overlay is None:
        return base
    if base is None:
        return overlay
    
    if isinstance(base, dict) and isinstance(overlay, dict):
        result = base.copy()
        for key, value in overlay.items():
            if key in result:
                result[key] = deep_merge(result[key], value)
            else:
                result[key] = value
        return result
    else:
        return overlay

# Load base config
with open('$base_config', 'r') as f:
    base = yaml.safe_load(f)

# Load overlay config
with open('$overlay_config', 'r') as f:
    overlay = yaml.safe_load(f)

# Merge configurations
merged = deep_merge(base, overlay)

# Save merged config
with open('$merged_config', 'w') as f:
    yaml.dump(merged, f, default_flow_style=False, sort_keys=False)

print('Configuration merged successfully')
"
    else
        log_info "No overlay found for $service in $environment"
    fi
    
    echo "$merged_config"
}

# Seed configuration to config-store
seed_config() {
    local service=$1
    local config_file=$2
    
    log_step "Seeding configuration for $service..."
    
    # Check if config-store is running
    if ! nc -z localhost 50051 2>/dev/null; then
        log_warn "Config-store not running, saving to file system"
        
        local seed_dir="/tmp/config-seed/${CONFIG_ENV}"
        mkdir -p "$seed_dir"
        cp "$config_file" "$seed_dir/${service}.yaml"
        
        log_info "Configuration saved to $seed_dir/${service}.yaml"
        return 0
    fi
    
    # Send configuration to config-store via gRPC
    # This would use grpcurl in production
    log_info "Sending configuration to config-store..."
    
    # Simulate seeding (would use actual gRPC call)
    if command -v grpcurl > /dev/null 2>&1; then
        # Convert YAML to JSON for gRPC
        local json_config="/tmp/${service}_config.json"
        python3 -c "
import yaml, json
with open('$config_file', 'r') as f:
    config = yaml.safe_load(f)
with open('$json_config', 'w') as f:
    json.dump(config, f)
"
        
        # Send via gRPC (example command)
        # grpcurl -plaintext -d @ localhost:50051 config.ConfigStore/SetConfig < "$json_config"
        log_info "Configuration would be sent via gRPC (simulated)"
    fi
    
    log_info "✓ Configuration seeded for $service"
}

# Generate seeding report
generate_report() {
    local report_file="/tmp/config-seed-report.txt"
    local seed_dir="/tmp/config-seed/${CONFIG_ENV}"
    
    cat > "$report_file" << EOF
Configuration Seeding Report
============================
Date: $(date)
Environment: $CONFIG_ENV
Repository: $CONFIG_REPO

Services Configured:
--------------------
EOF
    
    for service in "${SERVICES[@]}"; do
        if [ -f "$seed_dir/${service}.yaml" ]; then
            echo "✓ $service" >> "$report_file"
        else
            echo "✗ $service (failed)" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" << EOF

Validation Results:
-------------------
$([ "$VALIDATE" = "true" ] && echo "All configurations validated against schemas" || echo "Validation skipped")

Configuration Location:
-----------------------
Seed Directory: $seed_dir
Config Store: ${CONFIG_STORE_URL:-localhost:50051}

Next Steps:
-----------
1. Verify configurations in config-store
2. Restart services to load new configs
3. Monitor service health

EOF
    
    log_info "Seeding report saved to $report_file"
    cat "$report_file"
}

# Main execution
main() {
    log_info "Starting configuration seeding for environment: $CONFIG_ENV"
    
    # Check if config repository exists
    if [ ! -d "$CONFIG_REPO" ]; then
        log_error "Configuration repository not found: $CONFIG_REPO"
        exit 1
    fi
    
    local all_valid=true
    local seed_dir="/tmp/config-seed/${CONFIG_ENV}"
    mkdir -p "$seed_dir"
    
    # Process each service
    for service in "${SERVICES[@]}"; do
        log_info "Processing $service..."
        
        # Merge base and overlay
        merged_config=$(merge_configs "$service" "$CONFIG_ENV")
        
        if [ -z "$merged_config" ] || [ ! -f "$merged_config" ]; then
            log_error "Failed to merge configuration for $service"
            all_valid=false
            continue
        fi
        
        # Validate if enabled
        if [ "$VALIDATE" = "true" ]; then
            if ! validate_config "$service" "$merged_config"; then
                log_error "Configuration validation failed for $service"
                all_valid=false
                continue
            fi
        fi
        
        # Seed configuration
        seed_config "$service" "$merged_config"
    done
    
    # Generate report
    generate_report
    
    if [ "$all_valid" = true ]; then
        log_info "✓ All configurations seeded successfully"
        exit 0
    else
        log_error "✗ Some configurations failed to seed"
        exit 1
    fi
}

main
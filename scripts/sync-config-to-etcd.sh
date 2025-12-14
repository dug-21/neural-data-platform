#!/bin/bash
# Sync configuration files to etcd
# Usage: ./sync-config-to-etcd.sh [environment]

set -e

ETCD_ENDPOINT="${ETCD_ENDPOINT:-http://localhost:2379}"
ETCD_CONTAINER="${ETCD_CONTAINER:-etcd}"
ENVIRONMENT="${1:-development}"
CONFIG_DIR="${CONFIG_DIR:-./config}"

echo "Syncing config to etcd for environment: $ENVIRONMENT"

# Function to run etcdctl (local or docker)
run_etcdctl() {
    if command -v etcdctl &> /dev/null; then
        etcdctl --endpoints=$ETCD_ENDPOINT "$@"
    else
        docker exec $ETCD_CONTAINER etcdctl "$@"
    fi
}

# Function to sync a YAML file to etcd
sync_yaml_to_etcd() {
    local file=$1
    local service=$(basename $(dirname $file))
    local prefix="/$service"

    echo "Syncing $file to $prefix"

    # Convert YAML to JSON and flatten for etcd
    python3 -c "
import yaml
import json
import sys

def flatten(d, parent_key='', sep='/'):
    items = []
    for k, v in d.items():
        new_key = f'{parent_key}{sep}{k}' if parent_key else k
        if isinstance(v, dict):
            items.extend(flatten(v, new_key, sep=sep).items())
        else:
            items.append((new_key, v))
    return dict(items)

with open('$file') as f:
    data = yaml.safe_load(f)

flat = flatten(data)
for key, value in flat.items():
    print(f'$prefix/{key}|{json.dumps(value)}')
" | while IFS='|' read -r key value; do
        run_etcdctl put "$key" "$value"
    done
}

# Sync base configs
if [ -d "$CONFIG_DIR/base" ]; then
    for service_dir in "$CONFIG_DIR/base"/*/; do
        if [ -f "$service_dir/config.yaml" ]; then
            sync_yaml_to_etcd "$service_dir/config.yaml"
        fi
    done
fi

# Sync environment overlays (override base)
if [ -d "$CONFIG_DIR/overlays/$ENVIRONMENT" ]; then
    for service_dir in "$CONFIG_DIR/overlays/$ENVIRONMENT"/*/; do
        if [ -f "$service_dir/config.yaml" ]; then
            sync_yaml_to_etcd "$service_dir/config.yaml"
        fi
    done
fi

echo "Config sync complete!"

# Verify by listing keys
echo ""
echo "Current config keys:"
run_etcdctl get --prefix "/" --keys-only | head -20

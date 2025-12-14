#!/bin/bash
# AirGradient API Data Collector
# Fetches sensor measurements from the AirGradient Public API
# Docs: https://api.airgradient.com/public/docs/api/v1/

set -euo pipefail

# Configuration - set these via environment variables or edit defaults
AIRGRADIENT_API_TOKEN="${AIRGRADIENT_API_TOKEN:-}"
AIRGRADIENT_LOCATION_ID="${AIRGRADIENT_LOCATION_ID:-}"
OUTPUT_DIR="${OUTPUT_DIR:-./data/airgradient/api}"
API_BASE_URL="https://api.airgradient.com/public/api/v1"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

usage() {
    cat << EOF
AirGradient API Data Collector

Usage: $0 [OPTIONS]

Options:
    -t, --token TOKEN       API token (or set AIRGRADIENT_API_TOKEN env var)
    -l, --location ID       Location ID for specific location data
    -o, --output DIR        Output directory (default: ./data/airgradient/api)
    -a, --all               Fetch all locations with current measures
    -c, --config            Fetch configuration data (requires location ID)
    --local SERIAL          Fetch from local device (e.g., --local 84fce123abcd)
    -h, --help              Show this help message

Examples:
    # Fetch all locations
    $0 -t YOUR_TOKEN -a

    # Fetch specific location
    $0 -t YOUR_TOKEN -l 12345

    # Fetch from local device on same network
    $0 --local 84fce123abcd

    # Using environment variables
    export AIRGRADIENT_API_TOKEN=your_token
    $0 -a

Environment Variables:
    AIRGRADIENT_API_TOKEN   API token from AirGradient dashboard
    AIRGRADIENT_LOCATION_ID Default location ID
    OUTPUT_DIR              Output directory for data files

EOF
    exit 0
}

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

check_dependencies() {
    local missing=()
    for cmd in curl jq; do
        if ! command -v "$cmd" &> /dev/null; then
            missing+=("$cmd")
        fi
    done

    if [ ${#missing[@]} -ne 0 ]; then
        log_error "Missing required dependencies: ${missing[*]}"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            log_info "Install with: brew install ${missing[*]}"
        else
            log_info "Install with: apt-get install ${missing[*]}"
        fi
        exit 1
    fi
}

setup_output_dir() {
    mkdir -p "$OUTPUT_DIR"
    log_info "Output directory: $OUTPUT_DIR"
}

generate_filename() {
    local prefix=$1
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)
    echo "${OUTPUT_DIR}/${prefix}_${timestamp}.json"
}

fetch_all_locations() {
    if [ -z "$AIRGRADIENT_API_TOKEN" ]; then
        log_error "API token required. Use -t option or set AIRGRADIENT_API_TOKEN"
        exit 1
    fi

    local url="${API_BASE_URL}/locations/measures/current?token=${AIRGRADIENT_API_TOKEN}"
    local output_file
    output_file=$(generate_filename "all_locations")

    log_info "Fetching all locations with current measures..."

    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" "$url")
    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    if [ "$http_code" -eq 200 ]; then
        echo "$response" | jq '.' > "$output_file"
        log_info "Data saved to: $output_file"

        # Print summary
        local count
        count=$(echo "$response" | jq 'length')
        log_info "Retrieved data for $count location(s)"

        # Show sample of data structure
        echo -e "\n${GREEN}Sample data structure:${NC}"
        echo "$response" | jq '.[0] | keys' 2>/dev/null || echo "$response" | jq 'keys'
    else
        log_error "API request failed with HTTP $http_code"
        echo "$response" | jq '.' 2>/dev/null || echo "$response"
        exit 1
    fi
}

fetch_location() {
    local location_id=$1

    if [ -z "$AIRGRADIENT_API_TOKEN" ]; then
        log_error "API token required. Use -t option or set AIRGRADIENT_API_TOKEN"
        exit 1
    fi

    local url="${API_BASE_URL}/locations/${location_id}/measures/current?token=${AIRGRADIENT_API_TOKEN}"
    local output_file
    output_file=$(generate_filename "location_${location_id}")

    log_info "Fetching data for location: $location_id"

    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" "$url")
    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    if [ "$http_code" -eq 200 ]; then
        echo "$response" | jq '.' > "$output_file"
        log_info "Data saved to: $output_file"
        echo -e "\n${GREEN}Data preview:${NC}"
        echo "$response" | jq '.'
    else
        log_error "API request failed with HTTP $http_code"
        echo "$response" | jq '.' 2>/dev/null || echo "$response"
        exit 1
    fi
}

fetch_local_device() {
    local serial=$1
    local base_url="http://airgradient_${serial}.local"

    # Fetch current measures
    local measures_file
    measures_file=$(generate_filename "local_${serial}_measures")

    log_info "Fetching measures from local device: $serial"

    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" --connect-timeout 5 "${base_url}/measures/current" 2>/dev/null) || {
        log_error "Cannot connect to device. Ensure you're on the same network."
        exit 1
    }

    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    if [ "$http_code" -eq 200 ]; then
        echo "$response" | jq '.' > "$measures_file"
        log_info "Measures saved to: $measures_file"
        echo -e "\n${GREEN}Current measures:${NC}"
        echo "$response" | jq '.'
    else
        log_warn "Measures request failed with HTTP $http_code"
    fi

    # Also fetch config
    local config_file
    config_file=$(generate_filename "local_${serial}_config")

    log_info "Fetching config from local device: $serial"

    response=$(curl -s -w "\n%{http_code}" --connect-timeout 5 "${base_url}/config" 2>/dev/null) || true
    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    if [ "$http_code" -eq 200 ]; then
        echo "$response" | jq '.' > "$config_file"
        log_info "Config saved to: $config_file"
    else
        log_warn "Config request failed (may not be available on all firmware versions)"
    fi
}

# Parse command line arguments
FETCH_ALL=false
FETCH_LOCAL=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--token)
            AIRGRADIENT_API_TOKEN="$2"
            shift 2
            ;;
        -l|--location)
            AIRGRADIENT_LOCATION_ID="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -a|--all)
            FETCH_ALL=true
            shift
            ;;
        --local)
            FETCH_LOCAL="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            ;;
    esac
done

# Main execution
check_dependencies
setup_output_dir

if [ -n "$FETCH_LOCAL" ]; then
    fetch_local_device "$FETCH_LOCAL"
elif [ "$FETCH_ALL" = true ]; then
    fetch_all_locations
elif [ -n "$AIRGRADIENT_LOCATION_ID" ]; then
    fetch_location "$AIRGRADIENT_LOCATION_ID"
else
    log_error "No action specified. Use -a for all locations, -l for specific location, or --local for local device"
    usage
fi

log_info "Collection complete!"

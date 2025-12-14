#!/bin/bash
# AirGradient MQTT Event Listener
# Subscribes to MQTT topics and logs sensor data to files
# Docs: https://www.airgradient.com/support/kb-mqtt-conf/

set -euo pipefail

# Configuration - set via environment variables or edit defaults
MQTT_BROKER="${MQTT_BROKER:-}"
MQTT_PORT="${MQTT_PORT:-1883}"
MQTT_USERNAME="${MQTT_USERNAME:-}"
MQTT_PASSWORD="${MQTT_PASSWORD:-}"
MQTT_TOPIC="${MQTT_TOPIC:-airgradient/readings/#}"
MQTT_USE_TLS="${MQTT_USE_TLS:-false}"
OUTPUT_DIR="${OUTPUT_DIR:-./data/airgradient/mqtt}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

usage() {
    cat << EOF
AirGradient MQTT Event Listener

Usage: $0 [OPTIONS]

Options:
    -b, --broker HOST       MQTT broker hostname/IP (required)
    -p, --port PORT         MQTT broker port (default: 1883, or 8883 for TLS)
    -u, --username USER     MQTT username
    -P, --password PASS     MQTT password
    -t, --topic TOPIC       MQTT topic to subscribe (default: airgradient/readings/#)
    -o, --output DIR        Output directory (default: ./data/airgradient/mqtt)
    --tls                   Enable TLS/SSL connection
    --ca-cert FILE          CA certificate file for TLS
    -v, --verbose           Verbose output (show all messages)
    -d, --duration SECS     Run for specified duration then exit (default: infinite)
    -h, --help              Show this help message

MQTT Topic Structure:
    AirGradient publishes to: airgradient/readings/{SENSOR_SERIAL_NR}
    Use '#' wildcard to capture all sensors: airgradient/readings/#

Expected Payload Fields:
    wifi     - WiFi signal strength (dBm)
    ssid     - Connected network name
    atmp     - Ambient temperature (°C)
    rhum     - Relative humidity (%)
    rco2     - CO2 concentration (ppm)
    tvoc     - Total VOC (ppb)
    pm01     - PM1.0 (μg/m³)
    pm02     - PM2.5 (μg/m³)
    pm10     - PM10 (μg/m³)

Examples:
    # Basic connection
    $0 -b mqtt.example.com -u myuser -P mypass

    # TLS connection with custom port
    $0 -b mqtt.example.com -p 8883 --tls -u myuser -P mypass

    # Subscribe to specific sensor
    $0 -b mqtt.example.com -t "airgradient/readings/84fce123abcd"

    # Run for 1 hour and exit
    $0 -b mqtt.example.com -d 3600

Environment Variables:
    MQTT_BROKER      Broker hostname
    MQTT_PORT        Broker port
    MQTT_USERNAME    Username
    MQTT_PASSWORD    Password
    MQTT_TOPIC       Topic pattern
    MQTT_USE_TLS     Enable TLS (true/false)
    OUTPUT_DIR       Output directory

EOF
    exit 0
}

log_info() {
    echo -e "${GREEN}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') $1" >&2
}

log_data() {
    echo -e "${BLUE}[DATA]${NC} $(date '+%Y-%m-%d %H:%M:%S') $1"
}

check_dependencies() {
    if ! command -v mosquitto_sub &> /dev/null; then
        log_error "mosquitto_sub not found"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            log_info "Install with: brew install mosquitto"
        else
            log_info "Install with: apt-get install mosquitto-clients"
        fi
        exit 1
    fi

    if ! command -v jq &> /dev/null; then
        log_warn "jq not found - JSON formatting will be disabled"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            log_info "Install with: brew install jq"
        else
            log_info "Install with: apt-get install jq"
        fi
    fi
}

setup_output_dir() {
    mkdir -p "$OUTPUT_DIR"
    log_info "Output directory: $OUTPUT_DIR"
}

# Generate output filenames
get_log_file() {
    local date_str
    date_str=$(date +%Y%m%d)
    echo "${OUTPUT_DIR}/mqtt_events_${date_str}.jsonl"
}

get_raw_log_file() {
    local date_str
    date_str=$(date +%Y%m%d)
    echo "${OUTPUT_DIR}/mqtt_raw_${date_str}.log"
}

# Process incoming MQTT message
process_message() {
    local topic=$1
    local payload=$2
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Extract sensor serial from topic
    local sensor_id
    sensor_id=$(echo "$topic" | sed 's|airgradient/readings/||')

    # Create enriched JSON record
    local record
    if command -v jq &> /dev/null; then
        record=$(echo "$payload" | jq -c --arg ts "$timestamp" --arg topic "$topic" --arg sensor "$sensor_id" \
            '. + {_timestamp: $ts, _topic: $topic, _sensor_id: $sensor}' 2>/dev/null)

        if [ $? -ne 0 ]; then
            # Fallback if payload isn't valid JSON
            record="{\"_timestamp\":\"$timestamp\",\"_topic\":\"$topic\",\"_sensor_id\":\"$sensor_id\",\"_raw\":\"$payload\"}"
        fi
    else
        record="{\"_timestamp\":\"$timestamp\",\"_topic\":\"$topic\",\"_payload\":$payload}"
    fi

    # Append to JSONL file (one JSON object per line)
    local log_file
    log_file=$(get_log_file)
    echo "$record" >> "$log_file"

    # Also write raw log for debugging
    local raw_log
    raw_log=$(get_raw_log_file)
    echo "[$timestamp] $topic: $payload" >> "$raw_log"

    # Display if verbose
    if [ "$VERBOSE" = true ]; then
        if command -v jq &> /dev/null; then
            log_data "Topic: $topic"
            echo "$payload" | jq '.'
        else
            log_data "$topic: $payload"
        fi
    else
        # Just show sensor ID and key metrics
        if command -v jq &> /dev/null; then
            local summary
            summary=$(echo "$payload" | jq -r '"PM2.5: \(.pm02 // "N/A")μg/m³, CO2: \(.rco2 // "N/A")ppm, Temp: \(.atmp // "N/A")°C"' 2>/dev/null)
            log_data "[$sensor_id] $summary"
        else
            log_data "[$sensor_id] $payload"
        fi
    fi
}

# Signal handlers
cleanup() {
    log_info "Shutting down MQTT listener..."
    if [ -n "${MQTT_PID:-}" ]; then
        kill "$MQTT_PID" 2>/dev/null || true
    fi
    log_info "Listener stopped"
    exit 0
}

trap cleanup SIGINT SIGTERM

# Build mosquitto_sub command
build_mqtt_command() {
    local cmd="mosquitto_sub"
    cmd+=" -h $MQTT_BROKER"
    cmd+=" -p $MQTT_PORT"
    cmd+=" -t $MQTT_TOPIC"
    cmd+=" -v"  # Include topic in output

    if [ -n "$MQTT_USERNAME" ]; then
        cmd+=" -u $MQTT_USERNAME"
    fi

    if [ -n "$MQTT_PASSWORD" ]; then
        cmd+=" -P $MQTT_PASSWORD"
    fi

    if [ "$MQTT_USE_TLS" = true ]; then
        cmd+=" --capath /etc/ssl/certs"
        if [ -n "${CA_CERT:-}" ]; then
            cmd+=" --cafile $CA_CERT"
        fi
    fi

    echo "$cmd"
}

# Parse arguments
VERBOSE=false
DURATION=0
CA_CERT=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -b|--broker)
            MQTT_BROKER="$2"
            shift 2
            ;;
        -p|--port)
            MQTT_PORT="$2"
            shift 2
            ;;
        -u|--username)
            MQTT_USERNAME="$2"
            shift 2
            ;;
        -P|--password)
            MQTT_PASSWORD="$2"
            shift 2
            ;;
        -t|--topic)
            MQTT_TOPIC="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --tls)
            MQTT_USE_TLS=true
            [ "$MQTT_PORT" = "1883" ] && MQTT_PORT=8883
            shift
            ;;
        --ca-cert)
            CA_CERT="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -d|--duration)
            DURATION="$2"
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

# Validate required parameters
if [ -z "$MQTT_BROKER" ]; then
    log_error "MQTT broker is required. Use -b option or set MQTT_BROKER"
    usage
fi

# Main execution
check_dependencies
setup_output_dir

log_info "Starting AirGradient MQTT Listener"
log_info "Broker: $MQTT_BROKER:$MQTT_PORT"
log_info "Topic: $MQTT_TOPIC"
log_info "TLS: $MQTT_USE_TLS"
log_info "Output: $OUTPUT_DIR"
[ "$DURATION" -gt 0 ] && log_info "Duration: ${DURATION}s"

mqtt_cmd=$(build_mqtt_command)
log_info "Connecting to MQTT broker..."

# Create initial log files
touch "$(get_log_file)"
touch "$(get_raw_log_file)"

# Start duration timer if specified
if [ "$DURATION" -gt 0 ]; then
    (sleep "$DURATION" && kill $$ 2>/dev/null) &
fi

# Main listening loop
# mosquitto_sub outputs: "topic payload"
# We parse this and process each message
$mqtt_cmd 2>&1 | while IFS= read -r line; do
    # Skip connection messages
    if [[ "$line" == *"Connection"* ]] || [[ "$line" == *"Error"* ]]; then
        if [[ "$line" == *"Error"* ]]; then
            log_error "$line"
        else
            log_info "$line"
        fi
        continue
    fi

    # Parse topic and payload (mosquitto_sub -v format: "topic payload")
    topic=$(echo "$line" | cut -d' ' -f1)
    payload=$(echo "$line" | cut -d' ' -f2-)

    if [ -n "$topic" ] && [ -n "$payload" ]; then
        process_message "$topic" "$payload"
    fi
done

log_info "MQTT listener finished"
